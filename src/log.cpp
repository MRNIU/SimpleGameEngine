
/**
 * @file log.cpp
 * @brief 日志封装
 * @author Zone.N (Zone.Niuzh@hotmail.com)
 * @version 1.0
 * @date 2023-10-18
 * @copyright MIT LICENSE
 * https://github.com/Simple-XX/SimpleGameEngine
 * @par change log:
 * <table>
 * <tr><th>Date<th>Author<th>Description
 * <tr><td>2023-10-18<td>Zone.N<td>创建文件
 * </table>
 */

#include "log.h"

#include <spdlog/async.h>
#include <spdlog/sinks/basic_file_sink.h>
#include <spdlog/sinks/rotating_file_sink.h>
#include <spdlog/sinks/stdout_color_sinks.h>
#include <spdlog/spdlog.h>

#include <chrono>
#include <cstdio>

std::shared_ptr<spdlog::logger> SRLOG = nullptr;

/// @note 在 win 下使用时需要在程序结束时使用 `spdlog::shutdown()` 回收资源
void log_init(void) {
  try {
    spdlog::init_thread_pool(65536, 1);
    auto stdout_sink = std::make_shared<spdlog::sinks::stdout_color_sink_mt>();
    auto rotating_sink = std::make_shared<spdlog::sinks::rotating_file_sink_mt>(
        LOG_FILE_PATH, LOG_FILE_MAX_SIZE, LOG_FILE_MAX_COUNT);
    std::vector<spdlog::sink_ptr> sinks{stdout_sink, rotating_sink};
    auto logger = std::make_shared<spdlog::async_logger>(
        "multi_sink", sinks.begin(), sinks.end(), spdlog::thread_pool(),
        spdlog::async_overflow_policy::block);
    // [年-月-日 时:分:秒.毫秒] [文件名:行号] [日志级别以彩色大写输出 8
    // 字符右对齐] 内容
    logger->set_pattern("[%Y-%m-%d %H:%M:%S.%e] [%s:%# %!] [%^%l%$] %v");
    spdlog::register_logger(logger);
    spdlog::flush_on(spdlog::level::trace);

    SRLOG = spdlog::get("multi_sink");
  } catch (const spdlog::spdlog_ex &e) {
    std::printf("Log initialization failed: %s\n", e.what());
  }
}
