
/**
 * @file imgui_sdl2.h
 * @brief imgui-sdl2
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-11-15
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleGameEngine
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-11-15<td>Zone.N<td>创建文件
 * </table>
 */

#ifndef SIMPLEGAMEENGINE_SRC_PLATFORM_WINDOW_SYSTEM_IMGUI_SDL2_H
#define SIMPLEGAMEENGINE_SRC_PLATFORM_WINDOW_SYSTEM_IMGUI_SDL2_H

#include <SDL.h>
#include <SDL_syswm.h>
#include <imgui.h>
#include <imgui_impl_sdl2.h>

#include "log/log_system.h"

namespace simple_game_engine {
namespace platform {

class SDL2 {
 public:
  SDL_SysWMinfo wmi;
  SDL_Window* window = nullptr;

  SDL2(size_t width, size_t height) {
    if (SDL_Init(SDL_INIT_VIDEO) < 0) {
      SPDLOG_ERROR("SDL could not initialize. SDL_Error: %s\n", SDL_GetError());
      return;
    }

    window = SDL_CreateWindow("SimpleGameEngine", SDL_WINDOWPOS_UNDEFINED,
                              SDL_WINDOWPOS_UNDEFINED, width, height,
                              SDL_WINDOW_SHOWN);

    if (window == nullptr) {
      SPDLOG_ERROR("Window could not be created. SDL_Error: %s\n",
                   SDL_GetError());
      return;
    }

    SDL_VERSION(&wmi.version);
    if (!SDL_GetWindowWMInfo(window, &wmi)) {
      SPDLOG_ERROR("SDL_SysWMinfo could not be retrieved. SDL_Error: %s\n",
                   SDL_GetError());
      return;
    }
  }

  ~SDL2() {
    SDL_DestroyWindow(window);
    SDL_Quit();
  }
};

}  // namespace platform
}  // namespace simple_game_engine

#endif /* SIMPLEGAMEENGINE_SRC_PLATFORM_WINDOW_SYSTEM_IMGUI_SDL2_H */
