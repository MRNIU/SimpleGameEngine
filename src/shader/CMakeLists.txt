
# This file is a part of Simple-XX/SimpleGameEngine
# (https://github.com/Simple-XX/SimpleGameEngine).
#
# CMakeLists.txt for Simple-XX/SimpleGameEngine.

bgfx_compile_shader_to_header(
        TYPE VERTEX
        SHADERS ${CMAKE_CURRENT_SOURCE_DIR}/vs.sc ${CMAKE_CURRENT_SOURCE_DIR}/va.sc
        VARYING_DEF ${CMAKE_CURRENT_SOURCE_DIR}/varying.def.sc
        OUTPUT_DIR ${CMAKE_BINARY_DIR}/bin
        INCLUDE_DIRS ${bgfx.cmake_SOURCE_DIR}/bgfx/src
)

bgfx_compile_shader_to_header(
        TYPE FRAGMENT
        SHADERS ${CMAKE_CURRENT_SOURCE_DIR}/fs.sc
        VARYING_DEF ${CMAKE_CURRENT_SOURCE_DIR}/varying.def.sc
        OUTPUT_DIR ${CMAKE_BINARY_DIR}/bin
        INCLUDE_DIRS ${bgfx.cmake_SOURCE_DIR}/bgfx/src
)

add_library(shaders INTERFACE vs.sc fs.sc)
target_include_directories(shaders INTERFACE ${CMAKE_BINARY_DIR}/bin)

# 获取 shader 文件的文件名
get_target_property(shaders_SOURCES shaders SOURCES)
list(APPEND SHADER_FILE_BASENAME "")
foreach (shader_file ${shaders_SOURCES})
    get_filename_component(shader_file_base_name ${shader_file} NAME_WE)
    list(APPEND SHADER_FILE_BASENAME ${shader_file_base_name})
endforeach()


string(APPEND SHADERS_INCLUDE "")
string(APPEND EMBEDDEDSHADER_ARRAY "")

# 构造字符串
foreach (shader_include_file_name ${SHADER_FILE_BASENAME})
    string(APPEND SHADERS_INCLUDE 
"
// ${shader_include_file_name}
#include <${shader_include_file_name}.sc.glsl.bin.h>
#include <${shader_include_file_name}.sc.essl.bin.h>
#include <${shader_include_file_name}.sc.spv.bin.h>
#if defined(_WIN32)
#include <${shader_include_file_name}.sc.dx9.bin.h>
#include <${shader_include_file_name}.sc.dx11.bin.h>
#endif
#if __APPLE__
#include <${shader_include_file_name}.sc.mtl.bin.h>
#endif
"
)
    string(APPEND EMBEDDEDSHADER_ARRAY
"BGFX_EMBEDDED_SHADER(${shader_include_file_name}),
    "
)
endforeach()

# 生成 sherds.inc
configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/shaders.inc.in"
        "${CMAKE_CURRENT_SOURCE_DIR}/shaders.inc"
)
