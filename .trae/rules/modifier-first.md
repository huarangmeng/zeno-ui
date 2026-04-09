# Modifier First

## UI 语义必须以 Modifier 为唯一声明式入口
- 所有 UI 语义必须优先定义在 `Modifier` 层，禁止把声明式语义直接新增到 `Style`、`Node` 或渲染层作为另一份真相源。
- `Style` 只承担 resolved snapshot 的职责，用于布局和绘制阶段消费，不承载新的声明式语义定义。
- 如果某个能力需要跨布局、绘制和渲染传递，应先在 `Modifier` 定义语义，再通过 resolve 过程映射到 `Style` 或其他内部状态。
- 设计新能力时，优先遵循 `Modifier -> Style -> Scene` 的依赖流向，避免反向渗透或语义重复存储。
