use jvmti_bindings::classfile::{AttributeInfo, ClassFile};

struct CpBuilder {
    entries: Vec<Vec<u8>>,
}

impl CpBuilder {
    fn new() -> Self {
        Self { entries: Vec::new() }
    }

    fn push(&mut self, entry: Vec<u8>) -> u16 {
        self.entries.push(entry);
        self.entries.len() as u16
    }

    fn utf8(&mut self, s: &str) -> u16 {
        let mut entry = Vec::new();
        entry.push(1);
        entry.extend_from_slice(&(s.len() as u16).to_be_bytes());
        entry.extend_from_slice(s.as_bytes());
        self.push(entry)
    }

    fn class(&mut self, name_index: u16) -> u16 {
        let mut entry = Vec::new();
        entry.push(7);
        entry.extend_from_slice(&name_index.to_be_bytes());
        self.push(entry)
    }

    fn name_and_type(&mut self, name_index: u16, descriptor_index: u16) -> u16 {
        let mut entry = Vec::new();
        entry.push(12);
        entry.extend_from_slice(&name_index.to_be_bytes());
        entry.extend_from_slice(&descriptor_index.to_be_bytes());
        self.push(entry)
    }

    fn methodref(&mut self, class_index: u16, name_and_type_index: u16) -> u16 {
        let mut entry = Vec::new();
        entry.push(10);
        entry.extend_from_slice(&class_index.to_be_bytes());
        entry.extend_from_slice(&name_and_type_index.to_be_bytes());
        self.push(entry)
    }

    fn integer(&mut self, value: i32) -> u16 {
        let mut entry = Vec::new();
        entry.push(3);
        entry.extend_from_slice(&value.to_be_bytes());
        self.push(entry)
    }

    fn module(&mut self, name_index: u16) -> u16 {
        let mut entry = Vec::new();
        entry.push(19);
        entry.extend_from_slice(&name_index.to_be_bytes());
        self.push(entry)
    }

    fn package(&mut self, name_index: u16) -> u16 {
        let mut entry = Vec::new();
        entry.push(20);
        entry.extend_from_slice(&name_index.to_be_bytes());
        self.push(entry)
    }
}

fn u1(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}

fn u2(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn u4(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn push_attr(out: &mut Vec<u8>, name_index: u16, info: &[u8]) {
    u2(out, name_index);
    u4(out, info.len() as u32);
    out.extend_from_slice(info);
}

fn build_test_class() -> Vec<u8> {
    let mut cp = CpBuilder::new();

    let utf_test = cp.utf8("Test");
    let utf_object = cp.utf8("java/lang/Object");
    let class_test = cp.class(utf_test);
    let class_object = cp.class(utf_object);

    let utf_init = cp.utf8("<init>");
    let utf_void = cp.utf8("()V");
    let nat_init = cp.name_and_type(utf_init, utf_void);
    let _mref_object_init = cp.methodref(class_object, nat_init);

    let utf_field_name = cp.utf8("value");
    let utf_int_desc = cp.utf8("I");
    let const_int = cp.integer(123);

    let utf_code = cp.utf8("Code");
    let utf_lnt = cp.utf8("LineNumberTable");
    let utf_lvt = cp.utf8("LocalVariableTable");
    let utf_lvtt = cp.utf8("LocalVariableTypeTable");
    let utf_smt = cp.utf8("StackMapTable");

    let utf_source_file = cp.utf8("SourceFile");
    let utf_source_name = cp.utf8("Test.java");
    let utf_source_debug = cp.utf8("SourceDebugExtension");
    let utf_signature = cp.utf8("Signature");
    let utf_signature_val = cp.utf8("Ljava/lang/Object;");
    let utf_deprecated = cp.utf8("Deprecated");
    let utf_synthetic = cp.utf8("Synthetic");

    let utf_rva = cp.utf8("RuntimeVisibleAnnotations");
    let utf_ria = cp.utf8("RuntimeInvisibleAnnotations");
    let utf_rvpa = cp.utf8("RuntimeVisibleParameterAnnotations");
    let utf_ripa = cp.utf8("RuntimeInvisibleParameterAnnotations");
    let utf_rvta = cp.utf8("RuntimeVisibleTypeAnnotations");
    let utf_rita = cp.utf8("RuntimeInvisibleTypeAnnotations");
    let utf_annotation_default = cp.utf8("AnnotationDefault");
    let utf_bootstrap = cp.utf8("BootstrapMethods");
    let utf_method_params = cp.utf8("MethodParameters");
    let utf_exceptions = cp.utf8("Exceptions");
    let utf_inner_classes = cp.utf8("InnerClasses");
    let utf_enclosing = cp.utf8("EnclosingMethod");
    let utf_constant_value = cp.utf8("ConstantValue");

    let utf_module = cp.utf8("Module");
    let utf_module_packages = cp.utf8("ModulePackages");
    let utf_module_main = cp.utf8("ModuleMainClass");
    let utf_module_hashes = cp.utf8("ModuleHashes");
    let utf_module_target = cp.utf8("ModuleTarget");
    let utf_module_resolution = cp.utf8("ModuleResolution");

    let utf_nest_host = cp.utf8("NestHost");
    let utf_nest_members = cp.utf8("NestMembers");
    let utf_record = cp.utf8("Record");
    let utf_permitted = cp.utf8("PermittedSubclasses");

    let utf_unknown = cp.utf8("UnknownAttr");

    let utf_anno_type = cp.utf8("LMyAnno;");
    let utf_param_name = cp.utf8("arg0");
    let utf_component = cp.utf8("component");
    let utf_hash_alg = cp.utf8("SHA-256");
    let utf_target = cp.utf8("x86_64");
    let utf_module_name = cp.utf8("my.module");
    let utf_pkg_name = cp.utf8("my/pkg");

    let module_index = cp.module(utf_module_name);
    let package_index = cp.package(utf_pkg_name);

    let cp_count = (cp.entries.len() + 1) as u16;

    let mut bytes = Vec::new();
    u4(&mut bytes, 0xCAFEBABE);
    u2(&mut bytes, 0);
    u2(&mut bytes, 52);
    u2(&mut bytes, cp_count);
    for entry in cp.entries {
        bytes.extend_from_slice(&entry);
    }

    u2(&mut bytes, 0x0021);
    u2(&mut bytes, class_test);
    u2(&mut bytes, class_object);

    u2(&mut bytes, 0);

    u2(&mut bytes, 1);
    u2(&mut bytes, 0x0001);
    u2(&mut bytes, utf_field_name);
    u2(&mut bytes, utf_int_desc);
    u2(&mut bytes, 1);
    let mut cv_info = Vec::new();
    u2(&mut cv_info, const_int);
    push_attr(&mut bytes, utf_constant_value, &cv_info);

    u2(&mut bytes, 1);
    u2(&mut bytes, 0x0001);
    u2(&mut bytes, utf_init);
    u2(&mut bytes, utf_void);

    let mut method_attrs = Vec::new();

    let mut code_info = Vec::new();
    u2(&mut code_info, 1);
    u2(&mut code_info, 1);
    u4(&mut code_info, 1);
    u1(&mut code_info, 0xb1);
    u2(&mut code_info, 0);

    let mut code_sub_attrs = Vec::new();
    let mut lnt_info = Vec::new();
    u2(&mut lnt_info, 1);
    u2(&mut lnt_info, 0);
    u2(&mut lnt_info, 1);
    push_attr(&mut code_sub_attrs, utf_lnt, &lnt_info);

    let mut lvt_info = Vec::new();
    u2(&mut lvt_info, 1);
    u2(&mut lvt_info, 0);
    u2(&mut lvt_info, 1);
    u2(&mut lvt_info, utf_param_name);
    u2(&mut lvt_info, utf_int_desc);
    u2(&mut lvt_info, 0);
    push_attr(&mut code_sub_attrs, utf_lvt, &lvt_info);

    let mut lvtt_info = Vec::new();
    u2(&mut lvtt_info, 1);
    u2(&mut lvtt_info, 0);
    u2(&mut lvtt_info, 1);
    u2(&mut lvtt_info, utf_param_name);
    u2(&mut lvtt_info, utf_signature_val);
    u2(&mut lvtt_info, 0);
    push_attr(&mut code_sub_attrs, utf_lvtt, &lvtt_info);

    let mut smt_info = Vec::new();
    u2(&mut smt_info, 1);
    u1(&mut smt_info, 0);
    push_attr(&mut code_sub_attrs, utf_smt, &smt_info);

    u2(&mut code_info, 4);
    code_info.extend_from_slice(&code_sub_attrs);
    push_attr(&mut method_attrs, utf_code, &code_info);

    let mut exc_info = Vec::new();
    u2(&mut exc_info, 1);
    u2(&mut exc_info, class_object);
    push_attr(&mut method_attrs, utf_exceptions, &exc_info);

    let mut params_info = Vec::new();
    u1(&mut params_info, 1);
    u2(&mut params_info, utf_param_name);
    u2(&mut params_info, 0);
    push_attr(&mut method_attrs, utf_method_params, &params_info);

    let mut rvpa_info = Vec::new();
    u1(&mut rvpa_info, 1);
    u2(&mut rvpa_info, 0);
    push_attr(&mut method_attrs, utf_rvpa, &rvpa_info);

    let mut ripa_info = Vec::new();
    u1(&mut ripa_info, 1);
    u2(&mut ripa_info, 0);
    push_attr(&mut method_attrs, utf_ripa, &ripa_info);

    let mut ad_info = Vec::new();
    u1(&mut ad_info, b's');
    u2(&mut ad_info, utf_param_name);
    push_attr(&mut method_attrs, utf_annotation_default, &ad_info);

    u2(&mut bytes, 6);
    bytes.extend_from_slice(&method_attrs);

    let mut class_attrs = Vec::new();

    let mut sf_info = Vec::new();
    u2(&mut sf_info, utf_source_name);
    push_attr(&mut class_attrs, utf_source_file, &sf_info);

    let mut sde_info = Vec::new();
    sde_info.extend_from_slice(b"debug");
    push_attr(&mut class_attrs, utf_source_debug, &sde_info);

    let mut sig_info = Vec::new();
    u2(&mut sig_info, utf_signature_val);
    push_attr(&mut class_attrs, utf_signature, &sig_info);

    push_attr(&mut class_attrs, utf_deprecated, &[]);
    push_attr(&mut class_attrs, utf_synthetic, &[]);

    let mut rva_info = Vec::new();
    u2(&mut rva_info, 1);
    u2(&mut rva_info, utf_anno_type);
    u2(&mut rva_info, 0);
    push_attr(&mut class_attrs, utf_rva, &rva_info);

    let mut ria_info = Vec::new();
    u2(&mut ria_info, 1);
    u2(&mut ria_info, utf_anno_type);
    u2(&mut ria_info, 0);
    push_attr(&mut class_attrs, utf_ria, &ria_info);

    let mut rvta_info = Vec::new();
    u2(&mut rvta_info, 1);
    u1(&mut rvta_info, 0x13);
    u1(&mut rvta_info, 0);
    u2(&mut rvta_info, utf_anno_type);
    u2(&mut rvta_info, 0);
    push_attr(&mut class_attrs, utf_rvta, &rvta_info);

    let mut rita_info = Vec::new();
    u2(&mut rita_info, 1);
    u1(&mut rita_info, 0x13);
    u1(&mut rita_info, 0);
    u2(&mut rita_info, utf_anno_type);
    u2(&mut rita_info, 0);
    push_attr(&mut class_attrs, utf_rita, &rita_info);

    let mut bootstrap_info = Vec::new();
    u2(&mut bootstrap_info, 0);
    push_attr(&mut class_attrs, utf_bootstrap, &bootstrap_info);

    let mut inner_info = Vec::new();
    u2(&mut inner_info, 1);
    u2(&mut inner_info, class_test);
    u2(&mut inner_info, class_object);
    u2(&mut inner_info, utf_test);
    u2(&mut inner_info, 0x0001);
    push_attr(&mut class_attrs, utf_inner_classes, &inner_info);

    let mut enclosing_info = Vec::new();
    u2(&mut enclosing_info, class_object);
    u2(&mut enclosing_info, 0);
    push_attr(&mut class_attrs, utf_enclosing, &enclosing_info);

    let mut module_info = Vec::new();
    u2(&mut module_info, module_index);
    u2(&mut module_info, 0);
    u2(&mut module_info, 0);
    u2(&mut module_info, 0);
    u2(&mut module_info, 0);
    u2(&mut module_info, 0);
    u2(&mut module_info, 0);
    u2(&mut module_info, 0);
    push_attr(&mut class_attrs, utf_module, &module_info);

    let mut module_packages_info = Vec::new();
    u2(&mut module_packages_info, 1);
    u2(&mut module_packages_info, package_index);
    push_attr(&mut class_attrs, utf_module_packages, &module_packages_info);

    let mut module_main_info = Vec::new();
    u2(&mut module_main_info, class_test);
    push_attr(&mut class_attrs, utf_module_main, &module_main_info);

    let mut module_hashes_info = Vec::new();
    u2(&mut module_hashes_info, utf_hash_alg);
    u2(&mut module_hashes_info, 0);
    push_attr(&mut class_attrs, utf_module_hashes, &module_hashes_info);

    let mut module_target_info = Vec::new();
    u2(&mut module_target_info, utf_target);
    push_attr(&mut class_attrs, utf_module_target, &module_target_info);

    let mut module_resolution_info = Vec::new();
    u2(&mut module_resolution_info, 0);
    push_attr(&mut class_attrs, utf_module_resolution, &module_resolution_info);

    let mut nest_host_info = Vec::new();
    u2(&mut nest_host_info, class_object);
    push_attr(&mut class_attrs, utf_nest_host, &nest_host_info);

    let mut nest_members_info = Vec::new();
    u2(&mut nest_members_info, 1);
    u2(&mut nest_members_info, class_test);
    push_attr(&mut class_attrs, utf_nest_members, &nest_members_info);

    let mut record_info = Vec::new();
    u2(&mut record_info, 1);
    u2(&mut record_info, utf_component);
    u2(&mut record_info, utf_int_desc);
    u2(&mut record_info, 0);
    push_attr(&mut class_attrs, utf_record, &record_info);

    let mut permitted_info = Vec::new();
    u2(&mut permitted_info, 1);
    u2(&mut permitted_info, class_object);
    push_attr(&mut class_attrs, utf_permitted, &permitted_info);

    let mut unknown_info = Vec::new();
    unknown_info.extend_from_slice(b"data");
    push_attr(&mut class_attrs, utf_unknown, &unknown_info);

    u2(&mut bytes, 23);
    bytes.extend_from_slice(&class_attrs);

    bytes
}

#[test]
fn parses_all_attributes() {
    let bytes = build_test_class();
    let classfile = ClassFile::parse(&bytes).expect("parse class file");

    let class_attrs = &classfile.attributes;
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::SourceFile { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::SourceDebugExtension { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::Signature { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::Deprecated)));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::Synthetic)));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::RuntimeVisibleAnnotations { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::RuntimeInvisibleAnnotations { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::RuntimeVisibleTypeAnnotations { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::RuntimeInvisibleTypeAnnotations { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::BootstrapMethods { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::InnerClasses { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::EnclosingMethod { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::Module { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::ModulePackages { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::ModuleMainClass { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::ModuleHashes { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::ModuleTarget { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::ModuleResolution { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::NestHost { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::NestMembers { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::Record { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::PermittedSubclasses { .. })));
    assert!(class_attrs.iter().any(|a| matches!(a, AttributeInfo::Unknown { .. })));

    let field_attrs = &classfile.fields[0].attributes;
    assert!(field_attrs.iter().any(|a| matches!(a, AttributeInfo::ConstantValue { .. })));

    let method_attrs = &classfile.methods[0].attributes;
    assert!(method_attrs.iter().any(|a| matches!(a, AttributeInfo::Exceptions { .. })));
    assert!(method_attrs.iter().any(|a| matches!(a, AttributeInfo::MethodParameters { .. })));
    assert!(method_attrs.iter().any(|a| matches!(a, AttributeInfo::RuntimeVisibleParameterAnnotations { .. })));
    assert!(method_attrs.iter().any(|a| matches!(a, AttributeInfo::RuntimeInvisibleParameterAnnotations { .. })));
    assert!(method_attrs.iter().any(|a| matches!(a, AttributeInfo::AnnotationDefault { .. })));

    let code_attr = method_attrs
        .iter()
        .find_map(|a| if let AttributeInfo::Code(code) = a { Some(code) } else { None })
        .expect("code attr");

    assert!(code_attr.attributes.iter().any(|a| matches!(a, AttributeInfo::LineNumberTable { .. })));
    assert!(code_attr.attributes.iter().any(|a| matches!(a, AttributeInfo::LocalVariableTable { .. })));
    assert!(code_attr.attributes.iter().any(|a| matches!(a, AttributeInfo::LocalVariableTypeTable { .. })));
    assert!(code_attr.attributes.iter().any(|a| matches!(a, AttributeInfo::StackMapTable { .. })));
}
