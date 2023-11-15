
/**
 * @file app_base.h
 * @brief 应用程序基类
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

#ifndef SIMPLEGAMEENGINE_SRC_CORE_APPLICATION_APPLICATION_BASE_H
#define SIMPLEGAMEENGINE_SRC_CORE_APPLICATION_APPLICATION_BASE_H

namespace simple_game_engine {
namespace core {

class ApplicationBase {
 public:
  virtual int Initialize() = 0;
  virtual void Run() = 0;
  virtual void Tick() = 0;
  virtual void Render() = 0;
  virtual void Finalize() = 0;
  virtual void Quit() = 0;
  virtual bool IsQuit() = 0;
};

}  // namespace core
}  // namespace simple_game_engine

#endif /* SIMPLEGAMEENGINE_SRC_CORE_APPLICATION_APPLICATION_BASE_H */
