
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

add_library(platform
        bgfx/imgui_impl_bgfx.cpp
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
