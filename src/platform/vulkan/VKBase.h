
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

#ifndef SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_3_H
#define SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_3_H

#include "EasyVKStart.h"

namespace simple_game_engine {
namespace platform {

// 定义vulkan命名空间，之后会把Vulkan中一些基本对象的封装写在其中
namespace vulkan {
class graphicsBase {
  uint32_t apiVersion = VK_API_VERSION_1_0;
  VkInstance instance;
  VkPhysicalDevice physicalDevice;
  VkPhysicalDeviceProperties physicalDeviceProperties;
  VkPhysicalDeviceMemoryProperties physicalDeviceMemoryProperties;
  std::vector<VkPhysicalDevice> availablePhysicalDevices;

  VkDevice device;
  uint32_t queueFamilyIndex_graphics = VK_QUEUE_FAMILY_IGNORED;
  uint32_t queueFamilyIndex_presentation = VK_QUEUE_FAMILY_IGNORED;
  uint32_t queueFamilyIndex_compute = VK_QUEUE_FAMILY_IGNORED;
  VkQueue queue_graphics;
  VkQueue queue_presentation;
  VkQueue queue_compute;

  VkSurfaceKHR surface;
  std::vector<VkSurfaceFormatKHR> availableSurfaceFormats;

  VkSwapchainKHR swapchain;
  std::vector<VkImage> swapchainImages;
  std::vector<VkImageView> swapchainImageViews;
  VkSwapchainCreateInfoKHR swapchainCreateInfo = {};

  std::vector<const char*> instanceLayers;
  std::vector<const char*> instanceExtensions;
  std::vector<const char*> deviceExtensions;

  VkDebugUtilsMessengerEXT debugUtilsMessenger;

  // 静态变量
  static graphicsBase singleton;
  //--------------------
  graphicsBase() = default;
  graphicsBase(graphicsBase&&) = delete;
  ~graphicsBase() { /*待Ch1-4填充*/
  }
  Non - const函数 VkResult CreateSwapchain_Internal() { /*待Ch1-4填充*/
  }
  VkResult GetQueueFamilyIndices(VkPhysicalDevice physicalDevice,
                                 bool enableGraphicsQueue,
                                 bool enableComputeQueue,
                                 uint32_t (&queueFamilyIndices)[3]) {
    /*待Ch1-3填充*/
  }

  VkResult CreateDebugMessenger() { /*待Ch1-3填充*/
  }

 public:
  // Getter
  uint32_t ApiVersion() const { return apiVersion; }
  VkInstance Instance() const { return instance; }
  VkPhysicalDevice PhysicalDevice() const { return physicalDevice; }
  const VkPhysicalDeviceProperties& PhysicalDeviceProperties() const {
    return physicalDeviceProperties;
  }
  const VkPhysicalDeviceMemoryProperties& PhysicalDeviceMemoryProperties()
      const {
    return physicalDeviceMemoryProperties;
  }
  VkPhysicalDevice AvailablePhysicalDevice(uint32_t index) const {
    return availablePhysicalDevices[index];
  }
  uint32_t AvailablePhysicalDeviceCount() const {
    return uint32_t(availablePhysicalDevices.size());
  }

  VkDevice Device() const { return device; }
  uint32_t QueueFamilyIndex_Graphics() const {
    return queueFamilyIndex_graphics;
  }
  uint32_t QueueFamilyIndex_Presentation() const {
    return queueFamilyIndex_presentation;
  }
  uint32_t QueueFamilyIndex_Compute() const { return queueFamilyIndex_compute; }
  VkQueue Queue_Graphics() const { return queue_graphics; }
  VkQueue Queue_Presentation() const { return queue_presentation; }
  VkQueue Queue_Compute() const { return queue_compute; }

  VkSurfaceKHR Surface() const { return surface; }
  const VkFormat& AvailableSurfaceFormat(uint32_t index) const {
    return availableSurfaceFormats[index].format;
  }
  const VkColorSpaceKHR& AvailableSurfaceColorSpace(uint32_t index) const {
    return availableSurfaceFormats[index].colorSpace;
  }
  uint32_t AvailableSurfaceFormatCount() const {
    return uint32_t(availableSurfaceFormats.size());
  }

  VkSwapchainKHR Swapchain() const { return swapchain; }
  VkImage SwapchainImage(uint32_t index) const {
    return swapchainImages[index];
  }
  VkImageView SwapchainImageView(uint32_t index) const {
    return swapchainImageViews[index];
  }
  uint32_t SwapchainImageCount() const {
    return uint32_t(swapchainImages.size());
  }
  const VkSwapchainCreateInfoKHR& SwapchainCreateInfo() const {
    return swapchainCreateInfo;
  }

  const std::vector<const char*>& InstanceLayers() const {
    return instanceLayers;
  }
  const std::vector<const char*>& InstanceExtensions() const {
    return instanceExtensions;
  }
  const std::vector<const char*>& DeviceExtensions() const {
    return deviceExtensions;
  }

  // Const函数
  VkResult CheckInstanceLayers(std::span<const char*> layersToCheck) const {
    /*待Ch1-3填充*/
  }
  VkResult CheckInstanceExtensions(std::span<const char*> extensionsToCheck,
                                   const char* layerName = nullptr) const {
    /*待Ch1-3填充*/
  }
  VkResult CheckDeviceExtensions(std::span<const char*> extensionsToCheck,
                                 const char* layerName = nullptr) const {
    /*待Ch1-3填充*/
  }

  // Non-const函数
  void PushInstanceLayer(const char* layerName) {
    instanceLayers.push_back(layerName);
  }
  void PushInstanceExtension(const char* extensionName) {
    instanceExtensions.push_back(extensionName);
  }
  void PushDeviceExtension(const char* extensionName) {
    deviceExtensions.push_back(extensionName);
  }

  VkResult UseLatestApiVersion() { /*待Ch1-3填充*/
  }
  VkResult CreateInstance(const void* pNext = nullptr,
                          VkInstanceCreateFlags flags = 0) {
    /*待Ch1-3填充*/
  }
  void Surface(VkSurfaceKHR surface) {
    if (!this->surface) this->surface = surface;
  }
  VkResult GetPhysicalDevices() { /*待Ch1-3填充*/
  }
  VkResult DeterminePhysicalDevice(uint32_t deviceIndex = 0,
                                   bool enableGraphicsQueue,
                                   bool enableComputeQueue = true) {
    /*待Ch1-3填充*/
  }
  VkResult CreateDevice(const void* pNext = nullptr,
                        VkDeviceCreateFlags flags = 0) {
    /*待Ch1-3填充*/
  }
  VkResult GetSurfaceFormats() { /*待Ch1-4填充*/
  }
  VkResult SetSurfaceFormat(VkSurfaceFormatKHR surfaceFormat) {
    /*待Ch1-4填充*/
  }
  VkResult CreateSwapchain(bool limitFrameRate = true,
                           const void* pNext = nullptr,
                           VkSwapchainCreateFlagsKHR flags = 0) {
    /*待Ch1-4填充*/
  }

  void InstanceLayers(const std::vector<const char*>& layerNames) {
    instanceLayers = layerNames;
  }
  void InstanceExtensions(const std::vector<const char*>& extensionNames) {
    instanceExtensions = extensionNames;
  }
  void DeviceExtensions(const std::vector<const char*>& extensionNames) {
    deviceExtensions = extensionNames;
  }

  VkResult RecreateSwapchain() { /*待Ch1-4填充*/
  }
  // 静态函数
  static graphicsBase& Base() { return singleton; }
};
inline graphicsBase graphicsBase::singleton;

}  // namespace vulkan
}  // namespace platform
}  // namespace simple_game_engine

#endif  // SIMPLEGAMEENGINE_SRC_PLATFORM_VULKAN_3_H
