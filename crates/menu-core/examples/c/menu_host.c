#include "wii_menu_core.h"

#include <stdio.h>
#include <stdlib.h>

static char *read_file(const char *path, size_t *length)
{
    FILE *file = fopen(path, "rb");
    long size;
    char *data;
    if (file == NULL || fseek(file, 0, SEEK_END) != 0 || (size = ftell(file)) < 0 ||
        fseek(file, 0, SEEK_SET) != 0) {
        if (file != NULL) fclose(file);
        return NULL;
    }
    data = (char *)malloc((size_t)size);
    if (data == NULL || fread(data, 1, (size_t)size, file) != (size_t)size) {
        free(data);
        fclose(file);
        return NULL;
    }
    fclose(file);
    *length = (size_t)size;
    return data;
}

int main(int argc, char **argv)
{
    size_t length = 0, slot = 0, page = 0, page_count = 0, id_size = 0;
    uint32_t effect = 0;
    char *json;
    char *id;
    wii_menu_model *model = NULL;
    int32_t result;

    if (argc != 2) {
        fprintf(stderr, "usage: wii-menu-c-host <catalog.json>\n");
        return 2;
    }
    json = read_file(argv[1], &length);
    if (json == NULL) {
        fprintf(stderr, "could not read catalog: %s\n", argv[1]);
        return 2;
    }
    result = wii_menu_model_create_json((const uint8_t *)json, length, &model);
    free(json);
    if (result != WII_MENU_OK) {
        fprintf(stderr, "catalog rejected by shared menu core: %d\n", result);
        return 1;
    }
    result = wii_menu_model_apply(model, WII_MENU_MOVE_RIGHT, &effect);
    if (result == WII_MENU_OK)
        result = wii_menu_model_selection(model, &slot, &page, &page_count);
    if (result == WII_MENU_OK)
        result = wii_menu_model_selected_channel(model, NULL, 0, &id_size);
    if (result != WII_MENU_INVALID_ARGUMENT || id_size == 0) {
        fprintf(stderr, "menu selection query failed: %d\n", result);
        wii_menu_model_destroy(model);
        return 1;
    }
    id = (char *)malloc(id_size);
    if (id == NULL || wii_menu_model_selected_channel(model, id, id_size, &id_size) != WII_MENU_OK) {
        free(id);
        wii_menu_model_destroy(model);
        return 1;
    }
    printf("page %zu/%zu, slot %zu, channel %s, effect %u\n", page + 1, page_count,
        slot, id, effect);
    free(id);
    wii_menu_model_destroy(model);
    return 0;
}
