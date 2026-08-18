#!/usr/bin/env python3
"""Fail closed when ABI-equivalent wrapper arguments are dropped or reordered."""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
WRAPPERS = (ROOT / "src/jni_wrapper.rs", ROOT / "src/jvmti_wrapper.rs")

JNI_HANDLES = {
    "jobject",
    "jclass",
    "jstring",
    "jarray",
    "jthread",
    "jthrowable",
    "jweak",
    "jobjectArray",
    "jbooleanArray",
    "jbyteArray",
    "jcharArray",
    "jshortArray",
    "jintArray",
    "jlongArray",
    "jfloatArray",
    "jdoubleArray",
    "jmethodID",
    "jfieldID",
}

# These wrappers deliberately transform or delegate their arguments before the
# native call. Exact source contracts keep them inside the mechanical gate.
COMPOSITE_CONTRACTS = {
    "set_event_notification_mode": (
        r"mode\s*=\s*if\s+enable\s*\{\s*1\s*\}\s*else\s*\{\s*0\s*\}",
        r"set_mode_fn\(self\.env,\s*mode,\s*event_type,\s*thread\)",
    ),
    "create_raw_monitor_raw": (
        r"c_name\s*=\s*mutf8::encode_cstring\(name\)",
        r"create_fn\(self\.env,\s*c_name\.as_ptr\(\),\s*&mut\s+monitor\)",
    ),
    "get_named_module": (
        r"c_package\s*=\s*mutf8::encode_cstring\(package_name\)",
        r"get_module_fn\(self\.env,\s*class_loader,\s*c_package\.as_ptr\(\),\s*&mut\s+module\)",
    ),
    "add_module_exports": (
        r"c_package\s*=\s*mutf8::encode_cstring\(package\)",
        r"add_fn\(self\.env,\s*module,\s*c_package\.as_ptr\(\),\s*to_module\)",
    ),
    "add_module_opens": (
        r"c_package\s*=\s*mutf8::encode_cstring\(package\)",
        r"add_fn\(self\.env,\s*module,\s*c_package\.as_ptr\(\),\s*to_module\)",
    ),
    "get_system_property": (
        r"c_property\s*=\s*mutf8::encode_cstring\(property\)",
        r"get_fn\(self\.env,\s*c_property\.as_ptr\(\),\s*&mut\s+value_ptr\)",
    ),
    "module_can_read": (
        r"jvalue\s*\{\s*l:\s*other\s*\}",
        r"call_boolean_method\(module,\s*method,\s*&args\)",
    ),
    "module_is_exported_to": (
        r"module_package_access\(module,\s*package_name,\s*other,\s*c\"isExported\"\)",
    ),
    "module_is_open_to": (
        r"module_package_access\(module,\s*package_name,\s*other,\s*c\"isOpen\"\)",
    ),
    "get_method_id": (
        r"encode_cstring\(name\)",
        r"encode_cstring\(sig\)",
        r"get_method_id_cstr\(cls,\s*&c_name,\s*&c_sig\)",
    ),
    "get_static_method_id": (
        r"encode_cstring\(name\)",
        r"encode_cstring\(sig\)",
        r"get_static_method_id_cstr\(cls,\s*&c_name,\s*&c_sig\)",
    ),
    "get_field_id": (
        r"encode_cstring\(name\)",
        r"encode_cstring\(sig\)",
        r"get_field_id_cstr\(cls,\s*&c_name,\s*&c_sig\)",
    ),
    "get_static_field_id": (
        r"encode_cstring\(name\)",
        r"encode_cstring\(sig\)",
        r"get_static_field_id_cstr\(cls,\s*&c_name,\s*&c_sig\)",
    ),
    "set_system_property": (
        r"c_property\s*=\s*mutf8::encode_cstring\(property\)",
        r"c_value\s*=\s*mutf8::encode_cstring\(value\)",
        r"set_fn\(self\.env,\s*c_property\.as_ptr\(\),\s*c_value\.as_ptr\(\)\)",
    ),
    "add_to_bootstrap_class_loader_search": (
        r"c_segment\s*=\s*mutf8::encode_cstring\(segment\)",
        r"add_fn\(self\.env,\s*c_segment\.as_ptr\(\)\)",
    ),
    "add_to_system_class_loader_search": (
        r"c_segment\s*=\s*mutf8::encode_cstring\(segment\)",
        r"add_fn\(self\.env,\s*c_segment\.as_ptr\(\)\)",
    ),
    "set_native_method_prefix": (
        r"c_prefix\s*=\s*mutf8::encode_cstring\(prefix\)",
        r"set_fn\(self\.env,\s*c_prefix\.as_ptr\(\)\s+as\s+\*mut\s+_\)",
    ),
    "set_native_method_prefixes": (
        r"prefix_count\s*=\s*jint_len\(prefixes\.len\(\)\)",
        r"prefixes\s*\.iter\(\)\s*\.map\(\|prefix\|\s*mutf8::encode_cstring\(prefix\)\)",
        r"set_fn\(self\.env,\s*prefix_count,\s*prefix_ptrs\.as_mut_ptr\(\)\)",
    ),
}

# Macro-generated methods are not visible to the source-level function parser.
# Pin every macro family separately so a correct sibling cannot hide a broken
# boolean, static, nonvirtual, region, or lease variant.
MACRO_CONTRACTS = {
    "jni_function": (r"read_function_slot\(slot\)",),
    "jni_instance_call_a": (
        r"call\(self\.env,\s*object,\s*method,\s*arguments\.as_ptr\(\)\)",
    ),
    "jni_nonvirtual_call_a": (
        r"call\(self\.env,\s*object,\s*class,\s*method,\s*arguments\.as_ptr\(\)\)",
    ),
    "jni_nonvirtual_bool_call_a": (
        r"call\(self\.env,\s*object,\s*class,\s*method,\s*arguments\.as_ptr\(\)\)",
    ),
    "jni_static_call_a": (
        r"call\(self\.env,\s*class,\s*method,\s*arguments\.as_ptr\(\)\)",
    ),
    "jni_static_bool_call_a": (
        r"call\(self\.env,\s*class,\s*method,\s*arguments\.as_ptr\(\)\)",
    ),
    "jni_get_field": (r"get\(self\.env,\s*\$owner,\s*field\)",),
    "jni_get_bool_field": (r"get\(self\.env,\s*\$owner,\s*field\)",),
    "jni_set_field": (r"set\(self\.env,\s*\$owner,\s*field,\s*value\)",),
    "jni_set_bool_field": (
        r"set\(\s*self\.env,\s*\$owner,\s*field,\s*if\s+value\s*\{",
    ),
    "jni_new_primitive_array": (r"create\(self\.env,\s*length\)",),
    "jni_primitive_array_elements": (
        r"get\(self\.env,\s*array,\s*&mut\s+is_copy\)",
        r"PrimitiveArrayElements::new\(\s*self,\s*array,",
    ),
    "jni_primitive_array_region": (
        r"get\(self\.env,\s*array,\s*start,\s*length,\s*buffer\.as_mut_ptr\(\)\)",
        r"set\(self\.env,\s*array,\s*start,\s*length,\s*buffer\.as_ptr\(\)\)",
    ),
}


def fail(message: str) -> None:
    raise SystemExit(f"wrapper forwarding gate failed: {message}")


def matching(text: str, start: int, opening: str, closing: str) -> int:
    depth = 0
    state: str | None = None
    i = start
    while i < len(text):
        char = text[i]
        following = text[i + 1] if i + 1 < len(text) else ""
        if state == "line":
            if char == "\n":
                state = None
        elif state == "block":
            if char == "*" and following == "/":
                state = None
                i += 1
        elif state == "string":
            if char == "\\":
                i += 1
            elif char == '"':
                state = None
        elif char == "/" and following == "/":
            state = "line"
            i += 1
        elif char == "/" and following == "*":
            state = "block"
            i += 1
        elif char == '"':
            state = "string"
        elif char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return i
        i += 1
    fail(f"unterminated {opening}{closing} group at byte {start}")
    raise AssertionError("unreachable")


def split_top_level(source: str) -> list[str]:
    parts: list[str] = []
    start = 0
    levels = {"(": 0, "[": 0, "{": 0, "<": 0}
    closes = {")": "(", "]": "[", "}": "{", ">": "<"}
    for index, char in enumerate(source):
        if char in levels:
            levels[char] += 1
        elif char in closes and levels[closes[char]]:
            levels[closes[char]] -= 1
        elif char == "," and not any(levels.values()):
            parts.append(source[start:index].strip())
            start = index + 1
    parts.append(source[start:].strip())
    return [part for part in parts if part]


def abi_type(rust_type: str) -> str:
    handle = re.fullmatch(r"jni::(\w+)", rust_type)
    if handle and handle.group(1) in JNI_HANDLES:
        return "jni-handle-pointer"
    return rust_type


def function_parts(text: str):
    pattern = re.compile(r"pub\s+(?:unsafe\s+)?fn\s+(\w+)\s*(?:<[^\{;]*?>\s*)?\(")
    for match in pattern.finditer(text):
        open_paren = text.find("(", match.start())
        close_paren = matching(text, open_paren, "(", ")")
        open_brace = text.find("{", close_paren)
        if open_brace < 0:
            fail(f"missing body for {match.group(1)}")
        close_brace = matching(text, open_brace, "{", "}")
        yield match, text[open_paren + 1 : close_paren], text[open_brace + 1 : close_brace]


def parameters(signature: str) -> list[tuple[str, str]]:
    result: list[tuple[str, str]] = []
    for parameter in split_top_level(signature):
        if parameter.startswith(("&self", "&mut self", "mut self")) or parameter == "self":
            continue
        parsed = re.match(r"(?:mut\s+)?(\w+)\s*:\s*(.*)", parameter, re.DOTALL)
        if parsed:
            result.append((parsed.group(1), re.sub(r"\s+", " ", parsed.group(2).strip())))
    return result


def direct_native_call(body: str) -> tuple[str, list[str]] | None:
    bound = re.search(
        r"let\s+(\w+)\s*=\s*(?:jni|jvmti)_function!\(self,\s*(\w+)\)\??\s*;",
        body,
    )
    if bound:
        variable, slot = bound.groups()
        call = re.search(rf"\b{re.escape(variable)}\s*\(", body[bound.end() :])
        if not call:
            fail(f"{slot} is loaded but never called")
        open_paren = body.find("(", bound.end() + call.start())
    else:
        call = re.search(r"\(\(\*vtable\)\.(\w+)\)\s*\(", body)
        if not call:
            return None
        slot = call.group(1)
        open_paren = body.find("(", call.end() - 1)
    close_paren = matching(body, open_paren, "(", ")")
    return slot, split_top_level(body[open_paren + 1 : close_paren])


def main() -> None:
    direct_checked = 0
    helper_checked = 0
    composite_seen: set[str] = set()
    for path in WRAPPERS:
        text = path.read_text()
        for match, signature, body in function_parts(text):
            name = match.group(1)
            params = parameters(signature)
            native_call = direct_native_call(body)
            contracts = COMPOSITE_CONTRACTS.get(name)
            if contracts is not None:
                for contract in contracts:
                    if re.search(contract, body, re.DOTALL) is None:
                        fail(f"{path.name}:{name} no longer satisfies {contract!r}")
                composite_seen.add(name)
                if native_call is not None:
                    direct_checked += 1
                else:
                    helper_checked += 1
                continue
            if native_call is None:
                helper_checked += 1
                for parameter_name, _rust_type in params:
                    if re.search(rf"\b{re.escape(parameter_name)}\b", body) is None:
                        fail(
                            f"{path.name}:{name} drops helper input "
                            f"{parameter_name!r} before delegation"
                        )
                continue

            direct_checked += 1
            slot, arguments = native_call
            positions: list[int] = []
            names: list[str] = []
            for parameter_name, _rust_type in params:
                matches = [
                    index
                    for index, argument in enumerate(arguments)
                    if re.search(rf"\b{re.escape(parameter_name)}\b", argument)
                ]
                if not matches:
                    fail(f"{path.name}:{name} drops {parameter_name!r} before {slot}")
                names.append(parameter_name)
                positions.append(matches[0])
            if positions != sorted(positions) or len(set(positions)) != len(positions):
                fail(
                    f"{path.name}:{name} reorders or aliases {names!r} "
                    f"before {slot}: positions={positions!r}"
                )

    missing_composites = set(COMPOSITE_CONTRACTS) - composite_seen
    if missing_composites:
        fail(f"stale composite contracts: {sorted(missing_composites)!r}")
    if direct_checked != 237:
        fail(
            "expected the reviewed 3.0 inventory of 237 direct methods, "
            f"found {direct_checked}"
        )
    if helper_checked != 99:
        fail(
            "expected the reviewed 3.0 inventory of 99 helper methods, "
            f"found {helper_checked}"
        )

    jni_source = (ROOT / "src/jni_wrapper.rs").read_text()
    for macro_name, contracts in MACRO_CONTRACTS.items():
        declaration = re.search(rf"macro_rules!\s+{re.escape(macro_name)}\s*\{{", jni_source)
        if declaration is None:
            fail(f"missing JNI wrapper macro {macro_name}")
        open_brace = jni_source.find("{", declaration.start())
        close_brace = matching(jni_source, open_brace, "{", "}")
        macro_body = jni_source[open_brace + 1 : close_brace]
        for contract in contracts:
            if re.search(contract, macro_body, re.DOTALL) is None:
                fail(f"{macro_name} no longer satisfies {contract!r}")

    print(
        "wrapper forwarding: 237 direct hand-written methods, "
        "99 helper methods, "
        f"{len(COMPOSITE_CONTRACTS)} transformed/delegated contracts, and "
        "13 JNI macro families verified"
    )


if __name__ == "__main__":
    main()
