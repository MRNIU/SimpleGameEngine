
/**
 * @file shaders.inc
 * @brief 导入内嵌 shader 头文件
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-11-24
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleGameEngine
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-11-24<td>Zone.N<td>创建文件
 * </table>
 */

#ifndef SIMPLEGAMEENGINE_SHADERS_INC
#define SIMPLEGAMEENGINE_SHADERS_INC

#include <bgfx/embedded_shader.h>

namespace simple_game_engine {
namespace shader {

// vs
#if defined(_WIN32) || defined(__linux__)
#include <vs.sc.dx11.bin.h>
#include <vs.sc.dx10.bin.h>
#endif
#include <vs.sc.essl.bin.h>
#include <vs.sc.glsl.bin.h>
#include <vs.sc.spv.bin.h>
#if __APPLE__
#include <vs.sc.mtl.bin.h>
#endif

// fs
#if defined(_WIN32) || defined(__linux__)
#include <fs.sc.dx11.bin.h>
#include <fs.sc.dx10.bin.h>
#endif
#include <fs.sc.essl.bin.h>
#include <fs.sc.glsl.bin.h>
#include <fs.sc.spv.bin.h>
#if __APPLE__
#include <fs.sc.mtl.bin.h>
#endif

static const bgfx::EmbeddedShader kEmbeddedShaders[] = {
    BGFX_EMBEDDED_SHADER(vs),
    BGFX_EMBEDDED_SHADER(fs),
    
    BGFX_EMBEDDED_SHADER_END()
};

}  // namespace shader
}  // namespace simple_game_engine

#endif  // SIMPLEGAMEENGINE_SHADERS_INC
