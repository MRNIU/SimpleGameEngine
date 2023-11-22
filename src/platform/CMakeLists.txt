
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

add_library(platform
        #     vulkan/vk_initializers.cpp
        #     vulkan/vk_images.cpp
        #     vulkan/vk_descriptors.cpp
        #     vulkan/vk_pipelines.cpp
        #     vulkan/camera.cpp
        #     vulkan/vk_engine.cpp
        bgfx/helloworld_mt.cpp
        bgfx/helloworld.cpp

        window_system/window_system.cpp
        file_system/path.cpp
)

target_include_directories(platform PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
)

target_compile_options(platform PRIVATE
        ${DEFAULT_COMPILE_OPTIONS}
)

target_link_libraries(platform PRIVATE
        ${DEFAULT_LINK_LIB}
)
