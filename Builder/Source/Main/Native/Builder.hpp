#ifndef CODE_BUILDER_NATIVE_BUILDER_HPP
#define CODE_BUILDER_NATIVE_BUILDER_HPP

#include <jni.h>

#include <cstdint>

extern "C" {

uint32_t code_builder_abi_version(void);

const char *code_builder_core_version(void);

char *code_builder_create_project(const char *root, const char *spec);

char *code_builder_build_all(const char *root, const char *package_path,
                     const char *bundle_path, const char *key_path,
                     const char *key_password);

char *code_builder_build_progress(void);

void code_builder_build_expect(const char *timings);

void code_builder_build_stop(void);

char *code_builder_speak(const char *tag);

char *code_builder_terminal_docs(const char *tag);

char *code_builder_vibe(const char *request);

char *code_builder_verify_self(const char *package_path,
                       const char *expected_certificate);

char *code_builder_debugger_configure(const char *spec);

char *code_builder_create_key(const char *directory, const char *spec,
                      const char *password);

char *code_builder_bind_device(const char *secret);
char *code_builder_default_key(const char *directory);

char *code_builder_list_keys(const char *directory);

char *code_builder_delete_key(const char *path);

char *code_builder_check_key(const char *path, const char *password);

char *code_builder_list_projects(const char *directory);

char *code_builder_project_tree(const char *root);

char *code_builder_project_health(const char *root);

char *code_builder_lay_out(const char *name, const char *text);

char *code_builder_dependencies(const char *root);

char *code_builder_dependency_remove(const char *root, const char *name);

char *code_builder_manifest_facts(const char *root);

char *code_builder_manifest_set(const char *root, const char *field, const char *value);

char *code_builder_manifest_permission(const char *root, const char *name, bool wanted);

char *code_builder_symbols(const char *root, const char *needle);

char *code_builder_where_written(const char *root, const char *qualified);

char *code_builder_inspect_package(const char *path);

char *code_builder_check_project(const char *root);

char *code_builder_search_project(const char *root, const char *needle, bool sensitive,
                          bool whole_word);

char *code_builder_read_file(const char *root, const char *relative);

char *code_builder_write_file(const char *root, const char *relative, const char *contents);

char *code_builder_git_checkout(const char *pack_path, const char *head, const char *into);

char *code_builder_maven_plan(const char *root);

char *code_builder_project_repairs(const char *root);

char *code_builder_apply_repairs(const char *root, const char *repairs);

char *code_builder_digest_file(const char *path);

char *code_builder_pack_archive(const char *root, const char *names, const char *into,
                        const char *format, const char *password);

char *code_builder_adopt_project(const char *root);

char *code_builder_open_archive(const char *archive_path, const char *into,
                        const char *password);

char *code_builder_new_folder(const char *root, const char *relative);

char *code_builder_remove_path(const char *root, const char *relative,
                       const char *trash_root);

char *code_builder_rename_path(const char *root, const char *from, const char *to);

char *code_builder_copy_path(const char *root, const char *from, const char *to);

char *code_builder_build_unsigned(const char *root, const char *package_path,
                          const char *certificate, const char *quantum_seed);

char *code_builder_seal_package(const char *package_path, const char *sealed_path,
                        const char *certificate, const char *signature_v2,
                        const char *signature_v3, const char *signature_v31,
                        const char *signature_v32, const char *quantum_seed,
                        uint32_t min_sdk, uint32_t max_sdk);

char *code_builder_list_built(const char *directory);

char *code_builder_trash_send(const char *trash_root, const char *path);

char *code_builder_trash_list(const char *trash_root);

char *code_builder_trash_restore(const char *trash_root, const char *id);

char *code_builder_trash_purge(const char *trash_root, const char *id);

char *code_builder_trash_empty(const char *trash_root);

char *code_builder_set_icon(const char *root, const char *source);

void code_builder_string_free(char *value);

}

constexpr uint32_t kExpectedAbiVersion = 1;

#endif
