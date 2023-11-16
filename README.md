
# SimpleGameEngine

游戏引擎实现

## 架构

- 工具层 tool

    引擎最上层，用户界面

    GUI 系统
        MVVM

    assets 系统
        资产描述：json
        版本兼容：向前向后

    指令系统
        UNDO/REDO(invoke/revoke)：将指令记录在磁盘
        序列化/反序列化
        command UID
        add
        remove
        change
    
    Play in Editor

    插件

- 功能层 function

    显示、输入输出
    logic
    input
    camera
    motor
    character controller
    animation
    physics
    render
    network
    I/O
    memory gc
    thread

- 资源层 resource

    对资源的管理

- 核心层 core

    内存管理、数学、数据及数据结构等
    提供工具
    配置文件读写
    日志系统
    app 抽象
    高效的内存、数据结构

- 平台层 platform

    不同平台的兼容
    屏蔽不同平台的差异
    rhi
        opengl
        vulkan
    窗口系统
        windows
        linux
        osx

- 第三方 3rd

    第三方资源、插件，由 cmake 管理
    boost

## 参考

https://github.com/BoomingTech/Piccolo

https://github.com/netwarm007/GameEngineFromScratch

https://github.com/xiaoshichang/scarlett
