
/**
 * @file application.h
 * @brief 应用程序抽象
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

#ifndef SIMPLEGAMEENGINE_SRC_CORE_APPLICATION_APPLICATION_H
#define SIMPLEGAMEENGINE_SRC_CORE_APPLICATION_APPLICATION_H

#include "application_base.h"

// #include "Runtime/Core/Memory/MemoryManager.h"
// #include "Runtime/RHI/GraphicsMgr.h"
// #include "Runtime/Core/Time/TimeMgr.h"
// #include "Runtime/Core/Input/InputMgr.h"
// #include "Runtime/Core/Object/World.h"
// #include "Runtime/Core/UI/Director.h"
// #include "Runtime/Core/Font/FontMgr.h"
// #include "Runtime/Core/ScriptEngine/ScriptEngine.h"

namespace simple_game_engine {
namespace core {

class Application : public ApplicationBase {
 public:
  // GraphicsManager* mGraphicsManager_;
  // TimeMgr* mTimeMgr_;
  // InputMgr* mInputManager_;
  // FontMgr* mFontMgr_;
  // Director* mDirector_;
  // World* mWorld_;
  // IScriptEngine* mVM_;

  virtual int Initialize() override;
  virtual void Tick() override;
  virtual void Render() override;
  virtual void Run() override;
  virtual void Finalize() override;
  virtual void Quit() override;
  virtual bool IsQuit() override;

 private:
  bool is_quit;
};

// extern Application* GApp;

}  // namespace core
}  // namespace simple_game_engine

#endif /* SIMPLEGAMEENGINE_SRC_CORE_APPLICATION_APPLICATION_H */
