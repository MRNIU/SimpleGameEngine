
/**
 * @file app_base.cpp
 * @brief 应用程序基类实现
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

#include "application.h"

namespace simple_game_engine {
namespace core {

int Application::Initialize() {
  // mQuit = false;
  // mGraphicsManager = nullptr;
  // mDirector = nullptr;
  // mInputManager = nullptr;
  // mFontMgr = nullptr;
  // mWorld = nullptr;
  // mVM = nullptr;

  return 0;
}

void Application::Tick() {}

void Application::Render() {}

void Application::Run() {
  while (!IsQuit()) {
    Tick();
  }
}

void Application::Finalize() {}

bool Application::IsQuit() { return is_quit; }

void Application::Quit() { is_quit = true; }

}  // namespace core
}  // namespace simple_game_engine
