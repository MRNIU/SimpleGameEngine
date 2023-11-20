
/**
 * @file window_system.h
 * @brief 窗口系统抽象
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

#ifndef SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_1_H
#define SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_1_H

// 可能会用上的C++标准库
#include <chrono>
#include <concepts>
#include <format>
#include <fstream>
#include <functional>
#include <iostream>
#include <map>
#include <memory>
#include <numbers>
#include <numeric>
#include <span>
#include <sstream>
#include <stack>
#include <unordered_map>
#include <vector>

// GLM
#define GLM_FORCE_DEPTH_ZERO_TO_ONE
// 如果你惯用左手坐标系，在此定义GLM_FORCE_LEFT_HANDED
#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>

// stb_image.h
#include <stb_image.h>

// Vulkan
#include <vulkan/vulkan.h>

#endif  // SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_1_H
