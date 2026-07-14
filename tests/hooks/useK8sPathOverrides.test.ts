import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useK8sPathOverrides } from "../../src/hooks/useK8sPathOverrides";

const k8sMocks = vi.hoisted(() => ({
  validateK8sPath: vi.fn(),
}));

vi.mock("../../src/utils/k8s", () => ({
  validateK8sPath: k8sMocks.validateK8sPath,
}));

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
}

function createDeferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, resolve, reject };
}

describe("useK8sPathOverrides", () => {
  beforeEach(() => {
    k8sMocks.validateK8sPath.mockReset();
  });

  it("notifies consumers only when a path draft actually changes", () => {
    const onDraftChanged = vi.fn();
    const { result } = renderHook(() =>
      useK8sPathOverrides({ onDraftChanged }),
    );

    act(() => {
      result.current.setPath("kubectl", "");
      result.current.setPath("kubectl", "/opt/kubectl");
    });

    expect(onDraftChanged).toHaveBeenCalledTimes(1);
  });

  it("invalidates an in-flight validation when its draft is edited", async () => {
    const validation = createDeferred<void>();
    k8sMocks.validateK8sPath.mockReturnValue(validation.promise);
    const onApplied = vi.fn();
    const { result } = renderHook(() => useK8sPathOverrides({ onApplied }));

    act(() => {
      result.current.setPath("kubectl", "/first/kubectl");
    });
    const pending = result.current.validatePath("kubectl");

    act(() => {
      result.current.setPath("kubectl", "/second/kubectl");
    });
    await act(async () => {
      validation.resolve();
    });

    await expect(pending).resolves.toEqual({ status: "stale" });
    expect(result.current.kubectlPath).toBe("/second/kubectl");
    expect(result.current.kubectlValidation).toEqual({ status: "idle" });
    expect(result.current.appliedOptions).toEqual({});
    expect(onApplied).not.toHaveBeenCalled();
  });

  it("keeps invalid validation details and blocks application", async () => {
    k8sMocks.validateK8sPath.mockRejectedValue(new Error("not executable"));
    const { result } = renderHook(() => useK8sPathOverrides());

    act(() => {
      result.current.setPath("kubectl", "/bad/kubectl");
    });
    await act(async () => {
      await result.current.validatePath("kubectl");
    });

    expect(result.current.kubectlValidation).toEqual({
      status: "error",
      error: "not executable",
    });
    await expect(result.current.ensureApplied()).resolves.toEqual({
      status: "invalid",
    });
  });

  it("returns ready when the drafts already match the applied options", async () => {
    const { result } = renderHook(() => useK8sPathOverrides());

    await expect(result.current.ensureApplied()).resolves.toEqual({
      status: "ready",
      options: {},
    });
    expect(k8sMocks.validateK8sPath).not.toHaveBeenCalled();
  });

  it("validates initialized path overrides before reporting them ready", async () => {
    k8sMocks.validateK8sPath.mockResolvedValue(undefined);
    const onApplied = vi.fn();
    const { result } = renderHook(() => useK8sPathOverrides({ onApplied }));

    act(() => {
      result.current.initialize({
        kubectl_path: "/opt/kubectl",
        kubeconfig_path: "/tmp/kubeconfig",
      });
    });

    let ensured: Awaited<ReturnType<typeof result.current.ensureApplied>>;
    await act(async () => {
      ensured = await result.current.ensureApplied();
    });

    expect(ensured!).toEqual({
      status: "ready",
      options: {
        kubectl_path: "/opt/kubectl",
        kubeconfig_path: "/tmp/kubeconfig",
      },
    });
    expect(k8sMocks.validateK8sPath).toHaveBeenCalledWith(
      "/opt/kubectl",
      "kubectl",
    );
    expect(k8sMocks.validateK8sPath).toHaveBeenCalledWith(
      "/tmp/kubeconfig",
      "kubeconfig",
    );
    expect(onApplied).not.toHaveBeenCalled();
  });

  it("does not apply a changed pair when persisted preflight validation fails", async () => {
    k8sMocks.validateK8sPath.mockImplementation(
      (_path: string, kind: "kubectl" | "kubeconfig") =>
        kind === "kubectl"
          ? Promise.resolve(undefined)
          : Promise.reject(new Error("missing kubeconfig")),
    );
    const onApplied = vi.fn();
    const { result } = renderHook(() => useK8sPathOverrides({ onApplied }));

    act(() => {
      result.current.initialize({ kubeconfig_path: "/missing/config" });
      result.current.setPath("kubectl", "/opt/kubectl");
    });

    let ensured: Awaited<ReturnType<typeof result.current.ensureApplied>>;
    await act(async () => {
      ensured = await result.current.ensureApplied();
    });

    expect(ensured!).toEqual({ status: "invalid" });
    expect(onApplied).not.toHaveBeenCalled();
    expect(result.current.appliedOptions).toEqual({
      kubectl_path: undefined,
      kubeconfig_path: "/missing/config",
    });
  });

  it("validates and applies trimmed draft paths through ensureApplied", async () => {
    k8sMocks.validateK8sPath.mockResolvedValue(undefined);
    const onApplied = vi.fn();
    const { result } = renderHook(() => useK8sPathOverrides({ onApplied }));

    act(() => {
      result.current.setPath("kubectl", " /opt/kubectl ");
    });

    let ensured: Awaited<ReturnType<typeof result.current.ensureApplied>>;
    await act(async () => {
      ensured = await result.current.ensureApplied();
    });

    expect(ensured!).toEqual({
      status: "applied",
      options: { kubectl_path: "/opt/kubectl", kubeconfig_path: undefined },
    });
    expect(k8sMocks.validateK8sPath).toHaveBeenCalledWith(
      "/opt/kubectl",
      "kubectl",
    );
    expect(onApplied).toHaveBeenCalledTimes(1);
    expect(onApplied).toHaveBeenCalledWith({
      kubectl_path: "/opt/kubectl",
      kubeconfig_path: undefined,
    });
  });

  it.each(["kubectl", "kubeconfig"] as const)(
    "reconciles both valid blur results once when %s resolves first",
    async (firstKind) => {
      const kubectlValidation = createDeferred<void>();
      const kubeconfigValidation = createDeferred<void>();
      k8sMocks.validateK8sPath.mockImplementation(
        (_path: string, kind: "kubectl" | "kubeconfig") =>
          kind === "kubectl"
            ? kubectlValidation.promise
            : kubeconfigValidation.promise,
      );
      const onApplied = vi.fn();
      const { result } = renderHook(() => useK8sPathOverrides({ onApplied }));

      act(() => {
        result.current.setPath("kubectl", "/latest/kubectl");
        result.current.setPath("kubeconfig", "/latest/config");
      });
      const kubectlBlur = result.current.validatePath("kubectl");
      const kubeconfigBlur = result.current.validatePath("kubeconfig");

      const first =
        firstKind === "kubectl" ? kubectlValidation : kubeconfigValidation;
      const second =
        firstKind === "kubectl" ? kubeconfigValidation : kubectlValidation;
      await act(async () => {
        first.resolve();
      });
      expect(onApplied).not.toHaveBeenCalled();

      await act(async () => {
        second.resolve();
      });
      await Promise.all([kubectlBlur, kubeconfigBlur]);

      await waitFor(() => {
        expect(onApplied).toHaveBeenCalledTimes(1);
      });
      expect(onApplied).toHaveBeenCalledWith({
        kubectl_path: "/latest/kubectl",
        kubeconfig_path: "/latest/config",
      });
      expect(result.current.appliedOptions).toEqual({
        kubectl_path: "/latest/kubectl",
        kubeconfig_path: "/latest/config",
      });
    },
  );

  it("initializes and resets both draft and applied paths", () => {
    const { result } = renderHook(() => useK8sPathOverrides());

    act(() => {
      result.current.initialize({
        kubectl_path: "/initial/kubectl",
        kubeconfig_path: "/initial/config",
      });
    });
    expect(result.current.kubectlPath).toBe("/initial/kubectl");
    expect(result.current.kubeconfigPath).toBe("/initial/config");

    act(() => {
      result.current.reset();
    });
    expect(result.current.kubectlPath).toBe("");
    expect(result.current.kubeconfigPath).toBe("");
    expect(result.current.appliedOptions).toEqual({});
  });
});
