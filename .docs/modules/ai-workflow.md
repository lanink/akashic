# 模块：ai-workflow

> 生成时间：2026-11-23 12:05 +0800
> 覆盖范围：.docs/WORKFLOW.md, Claude.md, CODEBUDDY.md（按路径排序）
> 覆盖 hash：b5432c7e4d34（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
定义 AI Agent 在本项目中的开工流程、文档生成模板与更新义务；Claude.md 与 CODEBUDDY.md 分别是 Claude Code 与 WorkBuddy/CodeBuddy 的项目级自动加载规则入口。

## 对外接口
无函数接口。规则入口：
- `Claude.md`（Claude Code 读取）与 `CODEBUDDY.md`（WorkBuddy/CodeBuddy 读取）：内容均为“每次开始任务前，先读 .docs/WORKFLOW.md，严格按其规定执行”。
- `.docs/WORKFLOW.md` 第 1 节：读 `.docs/INDEX.md` → 读对应模块 md → 信息不足才读源码；禁止未查索引就全目录搜索。
- 第 3 节：模块 md 模板（生成时间、覆盖范围、覆盖 hash、可信度、状态 + 6 个固定章节，单份不超 300 行）。
- 第 5-6 节：小改动只重写受影响小节并刷新头部；结构性变更整模块重写并同步 INDEX.md；完成前逐项自查。
- 第 9 节：当 INDEX.md 或 .docs/modules/ 不存在时执行文档体系初始化。

## 内部关键流程
1. 会话启动 → Agent 平台自动加载 CODEBUDDY.md 或 Claude.md → 指令要求先读 WORKFLOW.md。
2. 每次任务 → 读 INDEX.md 定位模块 → 读模块 md → 必要时定向读源码。
3. 发现 md 与源码冲突 → 以源码为准刷新 md → 在回复中说明。
4. 改动接口/结构/流程 → 更新对应模块 md 的时间与 hash；结构性变更另更新 INDEX.md 的目录职责与依赖图。

## 依赖关系
- 依赖的外部模块：无。
- 被谁依赖：所有在本项目工作的 AI Agent；INDEX.md 与各模块 md 的格式由本模块规定。

## 常见改动点
- 调整开工流程或模板：由人类修改 WORKFLOW.md（第 8 节规定 Agent 不得自行修改）。
- 更换/增加 Agent 平台时，新增对应的项目规则文件，并保持与 Claude.md、CODEBUDDY.md 内容一致。
- 初始化或重建文档体系后，按第 9 节提示人类审阅 INDEX.md。

## 已知风险 / 易错点
- Claude.md 与 CODEBUDDY.md 是两份独立文件，修改其一必须同步另一份。
- WORKFLOW.md 第 8 节禁止 Agent 修改协议本身，只能提出建议。
- 本模块 md 的覆盖范围不含 INDEX.md 与其他模块 md，因此文档体系改动不会刷新本模块的覆盖 hash。
