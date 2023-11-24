
/**
 * @file singleton.h
 * @brief 单例模板
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

#ifndef SIMPLEGAMEENGINE_SRC_CORE_SINGLETON_SINGLETON_H
#define SIMPLEGAMEENGINE_SRC_CORE_SINGLETON_SINGLETON_H

namespace simple_game_engine {
namespace core {

template <typename T>
class Singleton {
 public:
  static T& GetInstance() {
    static T instance;
    return instance;
  }
};

}  // namespace core
}  // namespace simple_game_engine

#endif  // SIMPLEGAMEENGINE_SRC_CORE_SINGLETON_SINGLETON_H
