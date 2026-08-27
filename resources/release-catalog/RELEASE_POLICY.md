# Release-only 更新规则

1. Tabularis 与 Smart Proxy 的 main 出现代码更新后，对应提交必须有 GitHub Release。
2. 每个项目 Release 必须包含 Windows portable EXE 与 SHA256SUMS.txt。
3. 资源目录发生增加、删除、版本或哈希变化时，必须创建新的资源目录 Release；不得静默替换既有目录 Release。
4. 目录仅记录项目 Release 中已经发布并校验的资产，不在 Git 分支中提交二进制文件。
5. 两个项目及本目录保持 private，协作者仅限 lop-spec。
