
/**
 * @file path.cpp
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

#include "path.h"

#include <boost/predef/os.h>

#include <string>

#if (BOOST_OS_WINDOWS)
#include <stdlib.h>
#elif (BOOST_OS_SOLARIS)
#include <limits.h>
#include <stdlib.h>
#elif (BOOST_OS_LINUX)
#include <limits.h>
#include <unistd.h>
#elif (BOOST_OS_MACOS)
#include <mach-o/dyld.h>
#elif (BOOST_OS_BSD_FREE)
#include <sys/sysctl.h>
#include <sys/types.h>
#endif

namespace simple_game_engine {
namespace platform {

const std::filesystem::path Path::GetExecutablePath() {
#if (BOOST_OS_WINDOWS)
  char* exec_full_path;
  if (_get_pgmptr(&exec_full_path) != 0) {
    exec_full_path = "";
  }
#elif (BOOST_OS_SOLARIS)
  char exec_full_path[PATH_MAX];
  if (realpath(getexecname(), exec_full_path) == nullptr) {
    exec_full_path[0] = '\0';
  }
#elif (BOOST_OS_LINUX)
  char exec_full_path[PATH_MAX];
  auto len =
      ::readlink("/proc/self/exe", exec_full_path, sizeof(exec_full_path));
  if (len == -1 || len == sizeof(exec_full_path)) {
    len = 0;
  }
  exec_full_path[len] = '\0';
#elif (BOOST_OS_MACOS)
  char exec_full_path[PATH_MAX];
  uint32_t len = sizeof(exec_full_path);
  if (_NSGetExecutablePath(exec_full_path, &len) != 0) {
    exec_full_path[0] = '\0';  // buffer too small (!)
  } else {
    // resolve symlinks, ., .. if possible
    char* canonicalPath = realpath(exec_full_path, nullptr);
    if (canonicalPath != nullptr) {
      strncpy(exec_full_path, canonicalPath, len);
      free(canonicalPath);
    }
  }
#elif (BOOST_OS_BSD_FREE)
  char exec_full_path[2048];
  int mib[4];
  mib[0] = CTL_KERN;
  mib[1] = KERN_PROC;
  mib[2] = KERN_PROC_PATHNAME;
  mib[3] = -1;
  auto len = sizeof(exec_full_path);
  if (sysctl(mib, 4, exec_full_path, &len, nullptr, 0) != 0)
    exec_full_path[0] = '\0';
#endif
  return std::filesystem::path(exec_full_path);
}

const std::filesystem::path Path::GetRelativePath(
    const std::filesystem::path& directory,
    const std::filesystem::path& file_path) {
  return file_path.lexically_relative(directory);
}

const std::vector<std::string> Path::GetPathSegments(
    const std::filesystem::path& file_path) {
  std::vector<std::string> segments;
  for (auto iter = file_path.begin(); iter != file_path.end(); ++iter) {
    segments.emplace_back(iter->generic_string());
  }
  return segments;
}

const std::tuple<std::string, std::string, std::string> Path::GetFileExtensions(
    const std::filesystem::path& file_path) {
  return make_tuple(file_path.extension().generic_string(),
                    file_path.stem().extension().generic_string(),
                    file_path.stem().stem().extension().generic_string());
}

const std::string Path::GetFilePureName(const std::string file_full_name) {
  std::string file_pure_name = file_full_name;
  auto pos = file_full_name.find_first_of('.');
  if (pos != std::string::npos) {
    file_pure_name = file_full_name.substr(0, pos);
  }

  return file_pure_name;
}

}  // namespace platform
}  // namespace simple_game_engine
