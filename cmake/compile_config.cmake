
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# compile_config.cmake for Simple-XX/SimpleGameEngine.
# 配置信息

# 编译选项
list(APPEND DEFAULT_COMPILE_OPTIONS
        -Wall
        -Wextra
        $<$<CONFIG:Release>:-O3;-Werror>
        $<$<CONFIG:Debug>:-O0;-g;-ggdb>
)

list(APPEND DEFAULT_LINK_LIB
        spdlog::spdlog
        stb
        tinyobjloader
        Eigen
        Boost::headers
        Boost::json
        Vulkan::Vulkan
        Vulkan::Headers
        glm::glm
        glfw
)
