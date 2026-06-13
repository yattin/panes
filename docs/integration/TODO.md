# Panes改造 - 待办事项

> 单仓库维护：所有代码在 `panes/` 根目录。`claude-code-rust` 位于 `vendor/claude-code-rust/`。

## 高优先级

### [x] 完成i18n翻译
- [x] 翻译 app.json (617行)
- [x] 翻译 chat.json (559行)
- [ ] 人工审校关键术语
- [ ] 测试完整的中文界面

**预估时间**: 4-6小时
**影响**: 高（用户可见功能）

### [x] 修复claude-code-rust编译错误
- [x] 修复 repl.rs 中的类型推断错误（reqwest blocking 特性）
- [x] 验证编译通过
- [ ] 运行测试

**预估时间**: 2-3小时
**影响**: 高（阻塞集成）

## 中优先级

### [x] 启用claude-code-rust集成
- [x] 取消 panes/src-tauri/Cargo.toml 中的注释
- [x] 取消 engines/mod.rs 中的注释
- [x] 实现实际的API调用逻辑
- [ ] 测试消息收发（需配置 API key 后手动验证）

### [x] 前端集成
- [x] 在 types.ts 中添加 "claude-code-native" 类型
- [x] 更新 engineCapabilities.ts
- [ ] 测试引擎选择器（需启动应用验证）

## 低优先级

### [ ] 实施Agent架构
- [ ] 实现 Agent trait
- [ ] 创建 AgentRegistry
- [ ] 迁移现有引擎
- [ ] 前端UI更新
- [ ] 完整测试

**预估时间**: 8-12小时
**依赖**: claude-code-rust集成完成

### [ ] 测试和文档
- [ ] 添加i18n单元测试
- [ ] 添加引擎集成测试
- [ ] 更新用户文档
- [ ] 添加开发者文档

**预估时间**: 4-6小时

## 已完成 ✅

- [x] 创建workspace结构
- [x] 创建zh-CN资源文件目录
- [x] 翻译5个基础资源文件
- [x] 更新i18n配置
- [x] 添加语言选择器选项
- [x] 实现claude_code_native引擎
- [x] 设计Agent架构
- [x] 编写实施文档

---

**最后更新**: 2026-06-13
