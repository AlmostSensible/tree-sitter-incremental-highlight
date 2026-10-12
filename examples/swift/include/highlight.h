#ifndef IH_EXAMPLE_H
#define IH_EXAMPLE_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
// Example ABI. Handles are owned, must be freed once and used on one serial queue.
void *ih_document_new(const char *language);
char *ih_document_update(void *document, const uint8_t *source, size_t length,
                         uint64_t revision, bool force_full);
void ih_document_free(void *document);
void ih_string_free(char *text);
#endif
