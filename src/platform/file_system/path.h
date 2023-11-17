
/**
 * @file path.h
 * @brief 处理不同平台的路径差异
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

#ifndef SIMPLEGAMEENGINE_SRC_PLATFORM_FILE_SYSTEM_PATH_H
#define SIMPLEGAMEENGINE_SRC_PLATFORM_FILE_SYSTEM_PATH_H

#include <filesystem>
#include <string>
#include <tuple>
#include <vector>

namespace simple_game_engine {
namespace platform {

class Path {
 public:
  /**
   * 获取当前可执行文件的绝对路径
   * @return 路径
   */
  static const std::filesystem::path GetExecutablePath();

  static const std::filesystem::path GetRelativePath(
      const std::filesystem::path& directory,
      const std::filesystem::path& file_path);

  static const std::vector<std::string> GetPathSegments(
      const std::filesystem::path& file_path);

  static const std::tuple<std::string, std::string, std::string>
  GetFileExtensions(const std::filesystem::path& file_path);

  static const std::string GetFilePureName(const std::string);
};

}  // namespace platform
}  // namespace simple_game_engine

#endif  // SIMPLEGAMEENGINE_SRC_PLATFORM_FILE_SYSTEM_PATH_H
