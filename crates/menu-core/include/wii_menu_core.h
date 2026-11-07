/* Wii OS shared menu-state C ABI. Rust implementation is built as a static/dynamic library. */
#ifndef WII_MENU_CORE_H
#define WII_MENU_CORE_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32) && defined(WII_MENU_CORE_SHARED)
#  if defined(WII_MENU_CORE_BUILD)
#    define WII_MENU_API __declspec(dllexport)
#  else
#    define WII_MENU_API __declspec(dllimport)
#  endif
#elif defined(__GNUC__) && defined(WII_MENU_CORE_SHARED)
#  define WII_MENU_API __attribute__((visibility("default")))
#else
#  define WII_MENU_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct wii_menu_model wii_menu_model;

enum wii_menu_status { WII_MENU_OK = 0, WII_MENU_INVALID_ARGUMENT = 1,
  WII_MENU_INVALID_CATALOG = 2, WII_MENU_PANIC = 3 };
enum wii_menu_action { WII_MENU_MOVE_LEFT = 1, WII_MENU_MOVE_RIGHT, WII_MENU_MOVE_UP,
  WII_MENU_MOVE_DOWN, WII_MENU_SELECT, WII_MENU_START, WII_MENU_BACK,
  WII_MENU_SETTINGS, WII_MENU_ABOUT, WII_MENU_MAIL,
  WII_MENU_PAGE_PREVIOUS, WII_MENU_PAGE_NEXT, WII_MENU_HOME_MENU,
  WII_MENU_SD_CARD_MENU, WII_MENU_DATA_MANAGEMENT, WII_MENU_NETPLAY,
  WII_MENU_GAME_PACKAGER };
enum wii_menu_effect { WII_MENU_NO_EFFECT = 0, WII_MENU_HOVER = 1, WII_MENU_SELECT_CUE = 2,
  WII_MENU_BACK_CUE = 3, WII_MENU_ERROR_CUE = 4, WII_MENU_LAUNCH_CHANNEL = 5,
  WII_MENU_LAUNCH_SUCCEEDED = 6, WII_MENU_LAUNCH_FAILED = 7,
  WII_MENU_CHANNEL_MOVED = 8, WII_MENU_ENTERED_SAFE_MODE = 9 };

/* Catalog bytes are UTF-8 JSON using MenuCatalog schemaVersion 1. */
WII_MENU_API int32_t wii_menu_model_create_json(const uint8_t *data, size_t length,
  wii_menu_model **out_model);
WII_MENU_API void wii_menu_model_destroy(wii_menu_model *model);
WII_MENU_API int32_t wii_menu_model_apply(wii_menu_model *model, uint32_t action,
  uint32_t *out_effect);
WII_MENU_API int32_t wii_menu_model_selection(const wii_menu_model *model,
  size_t *out_slot, size_t *out_page, size_t *out_page_count);
/* On insufficient capacity, required receives bytes including NUL and no partial string is written. */
WII_MENU_API int32_t wii_menu_model_selected_channel(const wii_menu_model *model,
  char *output, size_t capacity, size_t *required);

#ifdef __cplusplus
}
#endif
#endif
