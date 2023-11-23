
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

bgfx_compile_shader_to_header(
        TYPE VERTEX
        SHADERS ${CMAKE_CURRENT_SOURCE_DIR}/v_simple.sc
        VARYING_DEF ${CMAKE_CURRENT_SOURCE_DIR}/varying.def.sc
        OUTPUT_DIR ${CMAKE_BINARY_DIR}/bin
        INCLUDE_DIRS ${bgfx.cmake_SOURCE_DIR}/bgfx/src
)

bgfx_compile_shader_to_header(
        TYPE FRAGMENT
        SHADERS ${CMAKE_CURRENT_SOURCE_DIR}/f_simple.sc
        VARYING_DEF ${CMAKE_CURRENT_SOURCE_DIR}/varying.def.sc
        OUTPUT_DIR ${CMAKE_BINARY_DIR}/bin
        INCLUDE_DIRS ${bgfx.cmake_SOURCE_DIR}/bgfx/src
)

add_library(shaders INTERFACE v_simple.sc f_simple.sc)
target_include_directories(shaders INTERFACE ${CMAKE_BINARY_DIR}/bin)
