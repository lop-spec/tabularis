export type NativeCliKind = "mongosh" | "redis-cli";

export interface NativeCliCommand {
  text: string;
  range: {
    start: number;
    end: number;
  };
}

function pushTrimmedCommand(
  source: string,
  start: number,
  end: number,
  commands: NativeCliCommand[],
): boolean {
  let trimmedStart = start;
  let trimmedEnd = end;
  while (trimmedStart < trimmedEnd && /\s/.test(source[trimmedStart])) {
    trimmedStart += 1;
  }
  while (trimmedEnd > trimmedStart && /\s/.test(source[trimmedEnd - 1])) {
    trimmedEnd -= 1;
  }

  if (trimmedStart >= trimmedEnd) return false;
  const text = source.slice(trimmedStart, trimmedEnd);
  if (!hasExecutableContent(text)) return false;
  commands.push({
    text,
    range: { start: trimmedStart, end: trimmedEnd },
  });
  return true;
}

function hasExecutableContent(text: string): boolean {
  let state: "normal" | "line-comment" | "block-comment" = "normal";
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    const next = text[index + 1];
    if (state === "line-comment") {
      if (char === "\n") state = "normal";
      continue;
    }
    if (state === "block-comment") {
      if (char === "*" && next === "/") {
        state = "normal";
        index += 1;
      }
      continue;
    }
    if (char === "/" && next === "/") {
      state = "line-comment";
      index += 1;
      continue;
    }
    if (char === "/" && next === "*") {
      state = "block-comment";
      index += 1;
      continue;
    }
    if (!/\s/.test(char) && char !== ";") return true;
  }
  return false;
}

function splitRedisCommands(source: string): NativeCliCommand[] {
  const commands: NativeCliCommand[] = [];
  let lineStart = 0;

  for (let index = 0; index <= source.length; index += 1) {
    if (index !== source.length && source[index] !== "\n") continue;
    const lineEnd =
      index > lineStart && source[index - 1] === "\r" ? index - 1 : index;
    pushTrimmedCommand(source, lineStart, lineEnd, commands);
    lineStart = index + 1;
  }

  return commands;
}

function previousSignificantIndex(source: string, from: number, floor: number): number {
  for (let index = from; index >= floor; index -= 1) {
    if (!/\s/.test(source[index])) return index;
  }
  return -1;
}

function nextSignificantIndex(source: string, from: number): number {
  for (let index = from; index < source.length; index += 1) {
    if (!/\s/.test(source[index])) return index;
  }
  return -1;
}

function previousWord(source: string, from: number, floor: number): string {
  const end = from + 1;
  let start = from;
  while (start >= floor && /[\w$]/.test(source[start])) start -= 1;
  start += 1;
  return source.slice(start, end);
}

function isRegexStart(source: string, index: number, commandStart: number): boolean {
  const previous = previousSignificantIndex(source, index - 1, commandStart);
  if (previous < 0) return true;
  if (/[([{=,:;!?&|+\-*%^~<>]/.test(source[previous])) return true;

  const word = previousWord(source, previous, commandStart);
  return /^(?:return|case|throw|delete|void|typeof|instanceof|in|of|yield|await)$/.test(
    word,
  );
}

function shouldSplitAtNewline(
  source: string,
  commandStart: number,
  codeEnd: number,
  newlineIndex: number,
): boolean {
  const previous = previousSignificantIndex(source, codeEnd - 1, commandStart);
  if (previous < 0) return false;

  const previousChar = source[previous];
  if (/[([{=.,:+\-*/%?&|!<>\\]/.test(previousChar)) return false;

  const next = nextSignificantIndex(source, newlineIndex + 1);
  if (next >= 0 && /[.)\]},?:+\-*/%&|!<>]/.test(source[next])) return false;

  const word = previousWord(source, previous, commandStart);
  if (/^(?:else|do|try|finally|case|throw|return|yield|await|new)$/.test(word)) {
    return false;
  }

  return true;
}

function splitMongoshCommands(source: string): NativeCliCommand[] {
  const commands: NativeCliCommand[] = [];
  let commandStart = 0;
  let state:
    | "normal"
    | "single"
    | "double"
    | "template"
    | "line-comment"
    | "block-comment"
    | "regex" = "normal";
  let escaped = false;
  let regexCharacterClass = false;
  let lineCommentStart = -1;
  let parentheses = 0;
  let brackets = 0;
  let braces = 0;

  for (let index = 0; index < source.length; index += 1) {
    const char = source[index];
    const next = source[index + 1];

    if (state === "line-comment") {
      if (char !== "\n") continue;
      state = "normal";
      if (
        parentheses === 0 &&
        brackets === 0 &&
        braces === 0 &&
        shouldSplitAtNewline(source, commandStart, lineCommentStart, index) &&
        pushTrimmedCommand(source, commandStart, index, commands)
      ) {
        commandStart = index + 1;
      }
      lineCommentStart = -1;
      continue;
    }

    if (state === "block-comment") {
      if (char === "*" && next === "/") {
        state = "normal";
        index += 1;
      }
      continue;
    }

    if (state === "single" || state === "double" || state === "template") {
      if (escaped) {
        escaped = false;
        continue;
      }
      if (char === "\\") {
        escaped = true;
        continue;
      }
      const closing =
        state === "single" ? "'" : state === "double" ? '"' : "`";
      if (char === closing) state = "normal";
      continue;
    }

    if (state === "regex") {
      if (escaped) {
        escaped = false;
        continue;
      }
      if (char === "\\") {
        escaped = true;
        continue;
      }
      if (char === "[") {
        regexCharacterClass = true;
        continue;
      }
      if (char === "]") {
        regexCharacterClass = false;
        continue;
      }
      if (char === "/" && !regexCharacterClass) state = "normal";
      continue;
    }

    if (char === "/" && next === "/") {
      state = "line-comment";
      lineCommentStart = index;
      index += 1;
      continue;
    }
    if (char === "/" && next === "*") {
      state = "block-comment";
      index += 1;
      continue;
    }
    if (char === "/" && isRegexStart(source, index, commandStart)) {
      state = "regex";
      regexCharacterClass = false;
      continue;
    }
    if (char === "'") {
      state = "single";
      escaped = false;
      continue;
    }
    if (char === '"') {
      state = "double";
      escaped = false;
      continue;
    }
    if (char === "`") {
      state = "template";
      escaped = false;
      continue;
    }

    if (char === "(") parentheses += 1;
    else if (char === ")") parentheses = Math.max(0, parentheses - 1);
    else if (char === "[") brackets += 1;
    else if (char === "]") brackets = Math.max(0, brackets - 1);
    else if (char === "{") braces += 1;
    else if (char === "}") braces = Math.max(0, braces - 1);

    if (
      char === ";" &&
      parentheses === 0 &&
      brackets === 0 &&
      braces === 0
    ) {
      pushTrimmedCommand(source, commandStart, index + 1, commands);
      commandStart = index + 1;
      continue;
    }

    if (
      char === "\n" &&
      parentheses === 0 &&
      brackets === 0 &&
      braces === 0 &&
      shouldSplitAtNewline(source, commandStart, index, index) &&
      pushTrimmedCommand(source, commandStart, index, commands)
    ) {
      commandStart = index + 1;
    }
  }

  pushTrimmedCommand(source, commandStart, source.length, commands);
  return commands;
}

export function splitNativeCliCommands(
  source: string,
  kind: NativeCliKind,
): NativeCliCommand[] {
  return kind === "redis-cli"
    ? splitRedisCommands(source)
    : splitMongoshCommands(source);
}

export function findNativeCliCommandAtOffset(
  commands: NativeCliCommand[],
  offset: number,
): NativeCliCommand | undefined {
  const direct = commands.find(
    (command) => offset >= command.range.start && offset <= command.range.end,
  );
  if (direct) return direct;
  return (
    commands.find((command) => offset < command.range.start) ??
    commands[commands.length - 1]
  );
}
