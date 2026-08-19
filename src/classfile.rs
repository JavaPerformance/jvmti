//! Class file parser for Java 8 through 28.
//!
//! This module provides a zero-dependency parser for `.class` files,
//! with typed decoding for JVMS-standard attributes through Java 28 and opaque
//! preservation of unknown and VM-specific attributes.

use std::borrow::Cow;
use std::cell::Cell;
use std::fmt;

use crate::mutf8;

/// Resource limits applied while parsing untrusted class-file bytes.
#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ClassFileParseLimits {
    max_annotation_nesting: usize,
    max_attribute_nesting: usize,
    max_input_bytes: usize,
    max_allocation_bytes: usize,
}

impl ClassFileParseLimits {
    /// Conservative defaults suitable for agent callbacks and offline tools.
    pub const fn new() -> Self {
        Self {
            max_annotation_nesting: 256,
            max_attribute_nesting: 64,
            max_input_bytes: 256 * 1024 * 1024,
            max_allocation_bytes: 256 * 1024 * 1024,
        }
    }

    /// Set the maximum nesting of annotation and annotation-array values.
    pub const fn with_max_annotation_nesting(mut self, maximum: usize) -> Self {
        self.max_annotation_nesting = maximum;
        self
    }

    /// Return the maximum nesting of annotation and annotation-array values.
    pub const fn max_annotation_nesting(self) -> usize {
        self.max_annotation_nesting
    }

    /// Set the maximum nesting of recursively parsed attributes.
    pub const fn with_max_attribute_nesting(mut self, maximum: usize) -> Self {
        self.max_attribute_nesting = maximum;
        self
    }

    /// Return the maximum nesting of recursively parsed attributes.
    pub const fn max_attribute_nesting(self) -> usize {
        self.max_attribute_nesting
    }

    /// Set the largest accepted class-file input.
    pub const fn with_max_input_bytes(mut self, maximum: usize) -> Self {
        self.max_input_bytes = maximum;
        self
    }

    /// Return the largest accepted class-file input.
    pub const fn max_input_bytes(self) -> usize {
        self.max_input_bytes
    }

    /// Set the cumulative capacity budget for parser-owned collections.
    pub const fn with_max_allocation_bytes(mut self, maximum: usize) -> Self {
        self.max_allocation_bytes = maximum;
        self
    }

    /// Return the cumulative capacity budget for parser-owned collections.
    pub const fn max_allocation_bytes(self) -> usize {
        self.max_allocation_bytes
    }
}

impl Default for ClassFileParseLimits {
    fn default() -> Self {
        Self::new()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: ConstantPool,
    pub access_flags: u16,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces: Vec<u16>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub attributes: Vec<AttributeInfo>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes: Vec<AttributeInfo>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes: Vec<AttributeInfo>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ConstantPool {
    entries: Vec<Option<CpInfo>>,
}

impl ConstantPool {
    pub fn get(&self, index: u16) -> Result<&CpInfo, ClassFileError> {
        if index == 0 {
            return Err(ClassFileError::InvalidConstantPoolIndex(index));
        }
        self.entries
            .get(index as usize)
            .and_then(|e| e.as_ref())
            .ok_or(ClassFileError::InvalidConstantPoolIndex(index))
    }

    pub fn get_utf8(&self, index: u16) -> Result<&str, ClassFileError> {
        self.get_java_string(index)?
            .as_str()
            .ok_or(ClassFileError::UnpairedSurrogate)
    }

    /// Return the exact Java string representation for a `CONSTANT_Utf8`.
    pub fn get_java_string(&self, index: u16) -> Result<&JavaString, ClassFileError> {
        match self.get(index)? {
            CpInfo::Utf8(value) => Ok(value),
            _ => Err(ClassFileError::InvalidConstantPoolIndex(index)),
        }
    }
}

/// A validated Java Modified UTF-8 value.
///
/// Most entries are Unicode scalar strings and borrow as `str`. Java also
/// permits unpaired UTF-16 surrogate code units; those remain available
/// exactly rather than being replaced or rejected.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaString {
    Unicode(String),
    Utf16WithUnpairedSurrogates(Vec<u16>),
}

impl JavaString {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Unicode(value) => Some(value),
            Self::Utf16WithUnpairedSurrogates(_) => None,
        }
    }

    pub fn to_utf16(&self) -> Cow<'_, [u16]> {
        match self {
            Self::Unicode(value) => Cow::Owned(value.encode_utf16().collect()),
            Self::Utf16WithUnpairedSurrogates(value) => Cow::Borrowed(value),
        }
    }

    pub fn to_modified_utf8(&self) -> Vec<u8> {
        match self {
            Self::Unicode(value) => mutf8::encode(value),
            Self::Utf16WithUnpairedSurrogates(value) => mutf8::encode_utf16(value),
        }
    }

    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        match self {
            Self::Unicode(value) => Cow::Borrowed(value),
            Self::Utf16WithUnpairedSurrogates(value) => Cow::Owned(String::from_utf16_lossy(value)),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum CpInfo {
    Utf8(JavaString),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Class {
        name_index: u16,
    },
    String {
        string_index: u16,
    },
    Fieldref {
        class_index: u16,
        name_and_type_index: u16,
    },
    Methodref {
        class_index: u16,
        name_and_type_index: u16,
    },
    InterfaceMethodref {
        class_index: u16,
        name_and_type_index: u16,
    },
    NameAndType {
        name_index: u16,
        descriptor_index: u16,
    },
    MethodHandle {
        reference_kind: u8,
        reference_index: u16,
    },
    MethodType {
        descriptor_index: u16,
    },
    Dynamic {
        bootstrap_method_attr_index: u16,
        name_and_type_index: u16,
    },
    InvokeDynamic {
        bootstrap_method_attr_index: u16,
        name_and_type_index: u16,
    },
    Module {
        name_index: u16,
    },
    Package {
        name_index: u16,
    },
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum AttributeInfo {
    ConstantValue {
        constantvalue_index: u16,
    },
    Code(CodeAttribute),
    StackMapTable(StackMapTableAttribute),
    Exceptions {
        exception_index_table: Vec<u16>,
    },
    InnerClasses {
        classes: Vec<InnerClassInfo>,
    },
    EnclosingMethod {
        class_index: u16,
        method_index: u16,
    },
    Synthetic,
    Signature {
        signature_index: u16,
    },
    SourceFile {
        sourcefile_index: u16,
    },
    SourceDebugExtension {
        debug_extension: Vec<u8>,
    },
    LineNumberTable {
        entries: Vec<LineNumberEntry>,
    },
    LocalVariableTable {
        entries: Vec<LocalVariableTableEntry>,
    },
    LocalVariableTypeTable {
        entries: Vec<LocalVariableTypeTableEntry>,
    },
    Deprecated,
    RuntimeVisibleAnnotations {
        annotations: Vec<Annotation>,
    },
    RuntimeInvisibleAnnotations {
        annotations: Vec<Annotation>,
    },
    RuntimeVisibleParameterAnnotations {
        parameter_annotations: Vec<Vec<Annotation>>,
    },
    RuntimeInvisibleParameterAnnotations {
        parameter_annotations: Vec<Vec<Annotation>>,
    },
    RuntimeVisibleTypeAnnotations {
        annotations: Vec<TypeAnnotation>,
    },
    RuntimeInvisibleTypeAnnotations {
        annotations: Vec<TypeAnnotation>,
    },
    AnnotationDefault {
        default_value: ElementValue,
    },
    BootstrapMethods {
        methods: Vec<BootstrapMethod>,
    },
    MethodParameters {
        parameters: Vec<MethodParameter>,
    },
    Module(ModuleAttribute),
    ModulePackages {
        packages: Vec<u16>,
    },
    ModuleMainClass {
        main_class_index: u16,
    },
    ModuleHashes {
        algorithm_index: u16,
        modules: Vec<ModuleHash>,
    },
    ModuleTarget {
        target_platform_index: u16,
    },
    ModuleResolution {
        resolution_flags: u16,
    },
    NestHost {
        host_class_index: u16,
    },
    NestMembers {
        classes: Vec<u16>,
    },
    Record {
        components: Vec<RecordComponent>,
    },
    PermittedSubclasses {
        classes: Vec<u16>,
    },
    Unknown {
        name: String,
        info: Vec<u8>,
    },
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct CodeAttribute {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code: Vec<u8>,
    pub exception_table: Vec<ExceptionTableEntry>,
    pub attributes: Vec<AttributeInfo>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ExceptionTableEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct StackMapTableAttribute {
    pub entries: Vec<StackMapFrame>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum StackMapFrame {
    Same {
        offset_delta: u16,
    },
    SameLocals1StackItem {
        offset_delta: u16,
        stack: VerificationTypeInfo,
    },
    SameLocals1StackItemExtended {
        offset_delta: u16,
        stack: VerificationTypeInfo,
    },
    Chop {
        offset_delta: u16,
        k: u8,
    },
    SameExtended {
        offset_delta: u16,
    },
    Append {
        offset_delta: u16,
        locals: Vec<VerificationTypeInfo>,
    },
    Full {
        offset_delta: u16,
        locals: Vec<VerificationTypeInfo>,
        stack: Vec<VerificationTypeInfo>,
    },
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum VerificationTypeInfo {
    Top,
    Integer,
    Float,
    Double,
    Long,
    Null,
    UninitializedThis,
    Object(u16),
    Uninitialized(u16),
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct LineNumberEntry {
    pub start_pc: u16,
    pub line_number: u16,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct LocalVariableTableEntry {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub index: u16,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct LocalVariableTypeTableEntry {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: u16,
    pub signature_index: u16,
    pub index: u16,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct InnerClassInfo {
    pub inner_class_info_index: u16,
    pub outer_class_info_index: u16,
    pub inner_name_index: u16,
    pub inner_class_access_flags: u16,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Annotation {
    pub type_index: u16,
    pub element_value_pairs: Vec<ElementValuePair>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ElementValuePair {
    pub element_name_index: u16,
    pub value: ElementValue,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ElementValue {
    Const {
        tag: u8,
        const_value_index: u16,
    },
    EnumConst {
        type_name_index: u16,
        const_name_index: u16,
    },
    ClassInfo {
        class_info_index: u16,
    },
    AnnotationValue(Annotation),
    ArrayValue(Vec<ElementValue>),
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TypeAnnotation {
    pub target_type: u8,
    pub target_info: TargetInfo,
    pub target_path: Vec<TypePathEntry>,
    pub type_index: u16,
    pub element_value_pairs: Vec<ElementValuePair>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum TargetInfo {
    TypeParameter {
        index: u8,
    },
    Supertype {
        index: u16,
    },
    TypeParameterBound {
        type_parameter_index: u8,
        bound_index: u8,
    },
    Empty,
    FormalParameter {
        index: u8,
    },
    Throws {
        index: u16,
    },
    Localvar {
        table: Vec<LocalVarTarget>,
    },
    Catch {
        exception_table_index: u16,
    },
    Offset {
        offset: u16,
    },
    TypeArgument {
        offset: u16,
        type_argument_index: u8,
    },
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct LocalVarTarget {
    pub start_pc: u16,
    pub length: u16,
    pub index: u16,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TypePathEntry {
    pub type_path_kind: u8,
    pub type_argument_index: u8,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BootstrapMethod {
    pub bootstrap_method_ref: u16,
    pub bootstrap_arguments: Vec<u16>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MethodParameter {
    pub name_index: u16,
    pub access_flags: u16,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ModuleAttribute {
    pub module_name_index: u16,
    pub module_flags: u16,
    pub module_version_index: u16,
    pub requires: Vec<ModuleRequires>,
    pub exports: Vec<ModuleExports>,
    pub opens: Vec<ModuleOpens>,
    pub uses: Vec<u16>,
    pub provides: Vec<ModuleProvides>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ModuleRequires {
    pub requires_index: u16,
    pub requires_flags: u16,
    pub requires_version_index: u16,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ModuleExports {
    pub exports_index: u16,
    pub exports_flags: u16,
    pub exports_to: Vec<u16>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ModuleOpens {
    pub opens_index: u16,
    pub opens_flags: u16,
    pub opens_to: Vec<u16>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ModuleProvides {
    pub provides_index: u16,
    pub provides_with: Vec<u16>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ModuleHash {
    pub module_name_index: u16,
    pub hash: Vec<u8>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct RecordComponent {
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes: Vec<AttributeInfo>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ClassFileError {
    UnexpectedEof,
    InvalidMagic(u32),
    InvalidConstantPoolIndex(u16),
    InvalidConstantPoolTag(u8),
    InvalidUtf8,
    UnpairedSurrogate,
    InvalidAttribute(String),
    LimitExceeded(&'static str),
}

impl fmt::Display for ClassFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassFileError::UnexpectedEof => write!(f, "unexpected end of file"),
            ClassFileError::InvalidMagic(m) => write!(f, "invalid magic: {m:#x}"),
            ClassFileError::InvalidConstantPoolIndex(i) => {
                write!(f, "invalid constant pool index: {i}")
            }
            ClassFileError::InvalidConstantPoolTag(t) => {
                write!(f, "invalid constant pool tag: {t}")
            }
            ClassFileError::InvalidUtf8 => write!(f, "invalid Modified UTF-8"),
            ClassFileError::UnpairedSurrogate => {
                write!(f, "Java string contains an unpaired UTF-16 surrogate")
            }
            ClassFileError::InvalidAttribute(name) => write!(f, "invalid attribute: {name}"),
            ClassFileError::LimitExceeded(limit) => {
                write!(f, "class-file parse limit exceeded: {limit}")
            }
        }
    }
}

impl std::error::Error for ClassFileError {}

struct Reader<'data, 'budget> {
    data: &'data [u8],
    pos: usize,
    limits: ClassFileParseLimits,
    remaining_allocation_bytes: &'budget Cell<usize>,
}

impl<'data, 'budget> Reader<'data, 'budget> {
    fn new(
        data: &'data [u8],
        limits: ClassFileParseLimits,
        remaining_allocation_bytes: &'budget Cell<usize>,
    ) -> Self {
        Self {
            data,
            pos: 0,
            limits,
            remaining_allocation_bytes,
        }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_u1(&mut self) -> Result<u8, ClassFileError> {
        if self.remaining() < 1 {
            return Err(ClassFileError::UnexpectedEof);
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u2(&mut self) -> Result<u16, ClassFileError> {
        if self.remaining() < 2 {
            return Err(ClassFileError::UnexpectedEof);
        }
        let v = u16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_u4(&mut self) -> Result<u32, ClassFileError> {
        if self.remaining() < 4 {
            return Err(ClassFileError::UnexpectedEof);
        }
        let v = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'data [u8], ClassFileError> {
        if self.remaining() < len {
            return Err(ClassFileError::UnexpectedEof);
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn claim_allocation(&self, bytes: usize) -> Result<(), ClassFileError> {
        let remaining = self.remaining_allocation_bytes.get();
        let Some(next) = remaining.checked_sub(bytes) else {
            return Err(ClassFileError::LimitExceeded("parser allocation budget"));
        };
        self.remaining_allocation_bytes.set(next);
        Ok(())
    }

    fn vec_with_capacity<T>(&self, count: usize) -> Result<Vec<T>, ClassFileError> {
        let bytes = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(ClassFileError::LimitExceeded("parser allocation budget"))?;
        self.claim_allocation(bytes)?;

        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| ClassFileError::LimitExceeded("parser allocation capacity"))?;
        Ok(values)
    }

    fn read_owned_bytes(&mut self, len: usize) -> Result<Vec<u8>, ClassFileError> {
        let bytes = self.read_bytes(len)?;
        let mut owned = self.vec_with_capacity(len)?;
        owned.extend_from_slice(bytes);
        Ok(owned)
    }
}

impl ClassFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, ClassFileError> {
        Self::parse_with_limits(bytes, ClassFileParseLimits::default())
    }

    /// Parse class bytes with explicit resource limits.
    pub fn parse_with_limits(
        bytes: &[u8],
        limits: ClassFileParseLimits,
    ) -> Result<Self, ClassFileError> {
        if bytes.len() > limits.max_input_bytes {
            return Err(ClassFileError::LimitExceeded("class-file input bytes"));
        }
        let remaining_allocation_bytes = Cell::new(limits.max_allocation_bytes);
        let mut r = Reader::new(bytes, limits, &remaining_allocation_bytes);
        let magic = r.read_u4()?;
        if magic != 0xCAFEBABE {
            return Err(ClassFileError::InvalidMagic(magic));
        }

        let minor_version = r.read_u2()?;
        let major_version = r.read_u2()?;

        let constant_pool = parse_constant_pool(&mut r)?;

        let access_flags = r.read_u2()?;
        let this_class = r.read_u2()?;
        let super_class = r.read_u2()?;

        let interfaces_count = r.read_u2()?;
        let mut interfaces = r.vec_with_capacity(interfaces_count as usize)?;
        for _ in 0..interfaces_count {
            interfaces.push(r.read_u2()?);
        }

        let fields_count = r.read_u2()?;
        let mut fields = r.vec_with_capacity(fields_count as usize)?;
        for _ in 0..fields_count {
            fields.push(parse_field(&mut r, &constant_pool)?);
        }

        let methods_count = r.read_u2()?;
        let mut methods = r.vec_with_capacity(methods_count as usize)?;
        for _ in 0..methods_count {
            methods.push(parse_method(&mut r, &constant_pool)?);
        }

        let attributes = parse_attributes(&mut r, &constant_pool, 1)?;
        if r.remaining() != 0 {
            r.claim_allocation("trailing class-file bytes".len())?;
            return Err(ClassFileError::InvalidAttribute(
                "trailing class-file bytes".to_owned(),
            ));
        }

        Ok(Self {
            minor_version,
            major_version,
            constant_pool,
            access_flags,
            this_class,
            super_class,
            interfaces,
            fields,
            methods,
            attributes,
        })
    }
}

fn parse_constant_pool(r: &mut Reader<'_, '_>) -> Result<ConstantPool, ClassFileError> {
    let count = r.read_u2()? as usize;
    let mut entries: Vec<Option<CpInfo>> = r.vec_with_capacity(count)?;
    entries.push(None); // index 0 is unused

    let mut i = 1;
    while i < count {
        let tag = r.read_u1()?;
        let entry = match tag {
            1 => {
                let len = r.read_u2()? as usize;
                let bytes = r.read_bytes(len)?;
                r.claim_allocation(len)?;
                let value = match mutf8::decode(bytes) {
                    Ok(value) => JavaString::Unicode(value),
                    Err(error) if error.kind() == mutf8::Mutf8ErrorKind::UnpairedSurrogate => {
                        let utf16_capacity = len
                            .checked_mul(std::mem::size_of::<u16>())
                            .ok_or(ClassFileError::LimitExceeded("parser allocation budget"))?;
                        r.claim_allocation(utf16_capacity)?;
                        JavaString::Utf16WithUnpairedSurrogates(
                            mutf8::decode_utf16(bytes).map_err(|_| ClassFileError::InvalidUtf8)?,
                        )
                    }
                    Err(_) => return Err(ClassFileError::InvalidUtf8),
                };
                CpInfo::Utf8(value)
            }
            3 => CpInfo::Integer(r.read_u4()? as i32),
            4 => {
                let bits = r.read_u4()?;
                CpInfo::Float(f32::from_bits(bits))
            }
            5 => {
                if i + 1 >= count {
                    return Err(ClassFileError::InvalidConstantPoolIndex((i + 1) as u16));
                }
                let high = r.read_u4()? as u64;
                let low = r.read_u4()? as u64;
                let value = ((high << 32) | low) as i64;
                entries.push(Some(CpInfo::Long(value)));
                entries.push(None);
                i += 2;
                continue;
            }
            6 => {
                if i + 1 >= count {
                    return Err(ClassFileError::InvalidConstantPoolIndex((i + 1) as u16));
                }
                let high = r.read_u4()? as u64;
                let low = r.read_u4()? as u64;
                let value = f64::from_bits((high << 32) | low);
                entries.push(Some(CpInfo::Double(value)));
                entries.push(None);
                i += 2;
                continue;
            }
            7 => CpInfo::Class {
                name_index: r.read_u2()?,
            },
            8 => CpInfo::String {
                string_index: r.read_u2()?,
            },
            9 => CpInfo::Fieldref {
                class_index: r.read_u2()?,
                name_and_type_index: r.read_u2()?,
            },
            10 => CpInfo::Methodref {
                class_index: r.read_u2()?,
                name_and_type_index: r.read_u2()?,
            },
            11 => CpInfo::InterfaceMethodref {
                class_index: r.read_u2()?,
                name_and_type_index: r.read_u2()?,
            },
            12 => CpInfo::NameAndType {
                name_index: r.read_u2()?,
                descriptor_index: r.read_u2()?,
            },
            15 => CpInfo::MethodHandle {
                reference_kind: r.read_u1()?,
                reference_index: r.read_u2()?,
            },
            16 => CpInfo::MethodType {
                descriptor_index: r.read_u2()?,
            },
            17 => CpInfo::Dynamic {
                bootstrap_method_attr_index: r.read_u2()?,
                name_and_type_index: r.read_u2()?,
            },
            18 => CpInfo::InvokeDynamic {
                bootstrap_method_attr_index: r.read_u2()?,
                name_and_type_index: r.read_u2()?,
            },
            19 => CpInfo::Module {
                name_index: r.read_u2()?,
            },
            20 => CpInfo::Package {
                name_index: r.read_u2()?,
            },
            _ => return Err(ClassFileError::InvalidConstantPoolTag(tag)),
        };

        entries.push(Some(entry));
        i += 1;
    }

    Ok(ConstantPool { entries })
}

fn parse_field(r: &mut Reader<'_, '_>, cp: &ConstantPool) -> Result<FieldInfo, ClassFileError> {
    let access_flags = r.read_u2()?;
    let name_index = r.read_u2()?;
    let descriptor_index = r.read_u2()?;
    let attributes = parse_attributes(r, cp, 1)?;
    Ok(FieldInfo {
        access_flags,
        name_index,
        descriptor_index,
        attributes,
    })
}

fn parse_method(r: &mut Reader<'_, '_>, cp: &ConstantPool) -> Result<MethodInfo, ClassFileError> {
    let access_flags = r.read_u2()?;
    let name_index = r.read_u2()?;
    let descriptor_index = r.read_u2()?;
    let attributes = parse_attributes(r, cp, 1)?;
    Ok(MethodInfo {
        access_flags,
        name_index,
        descriptor_index,
        attributes,
    })
}

fn parse_attributes(
    r: &mut Reader<'_, '_>,
    cp: &ConstantPool,
    depth: usize,
) -> Result<Vec<AttributeInfo>, ClassFileError> {
    if depth > r.limits.max_attribute_nesting {
        return Err(ClassFileError::LimitExceeded("attribute nesting"));
    }
    let count = r.read_u2()? as usize;
    let mut attrs = r.vec_with_capacity(count)?;
    for _ in 0..count {
        let name_index = r.read_u2()?;
        let length = r.read_u4()? as usize;
        let name = cp.get_utf8(name_index)?;
        let info_bytes = r.read_bytes(length)?;
        let mut sub = Reader::new(info_bytes, r.limits, r.remaining_allocation_bytes);

        let attr = match name {
            "ConstantValue" => {
                let constantvalue_index = sub.read_u2()?;
                AttributeInfo::ConstantValue {
                    constantvalue_index,
                }
            }
            "Code" => AttributeInfo::Code(parse_code_attribute(&mut sub, cp, depth)?),
            "StackMapTable" => AttributeInfo::StackMapTable(parse_stack_map_table(&mut sub)?),
            "Exceptions" => {
                let num = sub.read_u2()? as usize;
                let mut table = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    table.push(sub.read_u2()?);
                }
                AttributeInfo::Exceptions {
                    exception_index_table: table,
                }
            }
            "InnerClasses" => {
                let num = sub.read_u2()? as usize;
                let mut classes = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    classes.push(InnerClassInfo {
                        inner_class_info_index: sub.read_u2()?,
                        outer_class_info_index: sub.read_u2()?,
                        inner_name_index: sub.read_u2()?,
                        inner_class_access_flags: sub.read_u2()?,
                    });
                }
                AttributeInfo::InnerClasses { classes }
            }
            "EnclosingMethod" => {
                let class_index = sub.read_u2()?;
                let method_index = sub.read_u2()?;
                AttributeInfo::EnclosingMethod {
                    class_index,
                    method_index,
                }
            }
            "Synthetic" => AttributeInfo::Synthetic,
            "Signature" => {
                let signature_index = sub.read_u2()?;
                AttributeInfo::Signature { signature_index }
            }
            "SourceFile" => {
                let sourcefile_index = sub.read_u2()?;
                AttributeInfo::SourceFile { sourcefile_index }
            }
            "SourceDebugExtension" => {
                let data = sub.read_owned_bytes(sub.remaining())?;
                AttributeInfo::SourceDebugExtension {
                    debug_extension: data,
                }
            }
            "LineNumberTable" => {
                let num = sub.read_u2()? as usize;
                let mut entries = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    entries.push(LineNumberEntry {
                        start_pc: sub.read_u2()?,
                        line_number: sub.read_u2()?,
                    });
                }
                AttributeInfo::LineNumberTable { entries }
            }
            "LocalVariableTable" => {
                let num = sub.read_u2()? as usize;
                let mut entries = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    entries.push(LocalVariableTableEntry {
                        start_pc: sub.read_u2()?,
                        length: sub.read_u2()?,
                        name_index: sub.read_u2()?,
                        descriptor_index: sub.read_u2()?,
                        index: sub.read_u2()?,
                    });
                }
                AttributeInfo::LocalVariableTable { entries }
            }
            "LocalVariableTypeTable" => {
                let num = sub.read_u2()? as usize;
                let mut entries = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    entries.push(LocalVariableTypeTableEntry {
                        start_pc: sub.read_u2()?,
                        length: sub.read_u2()?,
                        name_index: sub.read_u2()?,
                        signature_index: sub.read_u2()?,
                        index: sub.read_u2()?,
                    });
                }
                AttributeInfo::LocalVariableTypeTable { entries }
            }
            "Deprecated" => AttributeInfo::Deprecated,
            "RuntimeVisibleAnnotations" => {
                let annotations = parse_annotations(&mut sub)?;
                AttributeInfo::RuntimeVisibleAnnotations { annotations }
            }
            "RuntimeInvisibleAnnotations" => {
                let annotations = parse_annotations(&mut sub)?;
                AttributeInfo::RuntimeInvisibleAnnotations { annotations }
            }
            "RuntimeVisibleParameterAnnotations" => {
                let parameter_annotations = parse_parameter_annotations(&mut sub)?;
                AttributeInfo::RuntimeVisibleParameterAnnotations {
                    parameter_annotations,
                }
            }
            "RuntimeInvisibleParameterAnnotations" => {
                let parameter_annotations = parse_parameter_annotations(&mut sub)?;
                AttributeInfo::RuntimeInvisibleParameterAnnotations {
                    parameter_annotations,
                }
            }
            "RuntimeVisibleTypeAnnotations" => {
                let annotations = parse_type_annotations(&mut sub)?;
                AttributeInfo::RuntimeVisibleTypeAnnotations { annotations }
            }
            "RuntimeInvisibleTypeAnnotations" => {
                let annotations = parse_type_annotations(&mut sub)?;
                AttributeInfo::RuntimeInvisibleTypeAnnotations { annotations }
            }
            "AnnotationDefault" => {
                let default_value = parse_element_value(&mut sub, 1)?;
                AttributeInfo::AnnotationDefault { default_value }
            }
            "BootstrapMethods" => {
                let num = sub.read_u2()? as usize;
                let mut methods = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    let method_ref = sub.read_u2()?;
                    let num_args = sub.read_u2()? as usize;
                    let mut args = sub.vec_with_capacity(num_args)?;
                    for _ in 0..num_args {
                        args.push(sub.read_u2()?);
                    }
                    methods.push(BootstrapMethod {
                        bootstrap_method_ref: method_ref,
                        bootstrap_arguments: args,
                    });
                }
                AttributeInfo::BootstrapMethods { methods }
            }
            "MethodParameters" => {
                let num = sub.read_u1()? as usize;
                let mut parameters = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    parameters.push(MethodParameter {
                        name_index: sub.read_u2()?,
                        access_flags: sub.read_u2()?,
                    });
                }
                AttributeInfo::MethodParameters { parameters }
            }
            "Module" => AttributeInfo::Module(parse_module_attribute(&mut sub)?),
            "ModulePackages" => {
                let num = sub.read_u2()? as usize;
                let mut packages = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    packages.push(sub.read_u2()?);
                }
                AttributeInfo::ModulePackages { packages }
            }
            "ModuleMainClass" => {
                let main_class_index = sub.read_u2()?;
                AttributeInfo::ModuleMainClass { main_class_index }
            }
            "ModuleHashes" => {
                let algorithm_index = sub.read_u2()?;
                let num = sub.read_u2()? as usize;
                let mut modules = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    let module_name_index = sub.read_u2()?;
                    let hash_len = sub.read_u2()? as usize;
                    let hash = sub.read_owned_bytes(hash_len)?;
                    modules.push(ModuleHash {
                        module_name_index,
                        hash,
                    });
                }
                AttributeInfo::ModuleHashes {
                    algorithm_index,
                    modules,
                }
            }
            "ModuleTarget" => {
                let target_platform_index = sub.read_u2()?;
                AttributeInfo::ModuleTarget {
                    target_platform_index,
                }
            }
            "ModuleResolution" => {
                let resolution_flags = sub.read_u2()?;
                AttributeInfo::ModuleResolution { resolution_flags }
            }
            "NestHost" => {
                let host_class_index = sub.read_u2()?;
                AttributeInfo::NestHost { host_class_index }
            }
            "NestMembers" => {
                let num = sub.read_u2()? as usize;
                let mut classes = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    classes.push(sub.read_u2()?);
                }
                AttributeInfo::NestMembers { classes }
            }
            "Record" => {
                let num = sub.read_u2()? as usize;
                let mut components = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    let name_index = sub.read_u2()?;
                    let descriptor_index = sub.read_u2()?;
                    let attributes = parse_attributes(
                        &mut sub,
                        cp,
                        depth
                            .checked_add(1)
                            .ok_or(ClassFileError::LimitExceeded("attribute nesting"))?,
                    )?;
                    components.push(RecordComponent {
                        name_index,
                        descriptor_index,
                        attributes,
                    });
                }
                AttributeInfo::Record { components }
            }
            "PermittedSubclasses" => {
                let num = sub.read_u2()? as usize;
                let mut classes = sub.vec_with_capacity(num)?;
                for _ in 0..num {
                    classes.push(sub.read_u2()?);
                }
                AttributeInfo::PermittedSubclasses { classes }
            }
            _ => {
                let info = sub.read_owned_bytes(sub.remaining())?;
                sub.claim_allocation(name.len())?;
                AttributeInfo::Unknown {
                    name: name.to_owned(),
                    info,
                }
            }
        };

        if sub.remaining() != 0 {
            sub.claim_allocation(name.len())?;
            return Err(ClassFileError::InvalidAttribute(name.to_owned()));
        }

        attrs.push(attr);
    }
    Ok(attrs)
}

fn parse_code_attribute(
    r: &mut Reader<'_, '_>,
    cp: &ConstantPool,
    attribute_depth: usize,
) -> Result<CodeAttribute, ClassFileError> {
    let max_stack = r.read_u2()?;
    let max_locals = r.read_u2()?;
    let code_length = r.read_u4()? as usize;
    let code = r.read_owned_bytes(code_length)?;
    let exception_table_length = r.read_u2()? as usize;
    let mut exception_table = r.vec_with_capacity(exception_table_length)?;
    for _ in 0..exception_table_length {
        exception_table.push(ExceptionTableEntry {
            start_pc: r.read_u2()?,
            end_pc: r.read_u2()?,
            handler_pc: r.read_u2()?,
            catch_type: r.read_u2()?,
        });
    }
    let attributes = parse_attributes(
        r,
        cp,
        attribute_depth
            .checked_add(1)
            .ok_or(ClassFileError::LimitExceeded("attribute nesting"))?,
    )?;
    Ok(CodeAttribute {
        max_stack,
        max_locals,
        code,
        exception_table,
        attributes,
    })
}

fn parse_stack_map_table(r: &mut Reader<'_, '_>) -> Result<StackMapTableAttribute, ClassFileError> {
    let num = r.read_u2()? as usize;
    let mut entries = r.vec_with_capacity(num)?;
    for _ in 0..num {
        let frame_type = r.read_u1()?;
        let frame = match frame_type {
            0..=63 => StackMapFrame::Same {
                offset_delta: frame_type as u16,
            },
            64..=127 => {
                let stack = parse_verification_type_info(r)?;
                StackMapFrame::SameLocals1StackItem {
                    offset_delta: (frame_type - 64) as u16,
                    stack,
                }
            }
            247 => {
                let offset_delta = r.read_u2()?;
                let stack = parse_verification_type_info(r)?;
                StackMapFrame::SameLocals1StackItemExtended {
                    offset_delta,
                    stack,
                }
            }
            248..=250 => {
                let offset_delta = r.read_u2()?;
                StackMapFrame::Chop {
                    offset_delta,
                    k: 251u8 - frame_type,
                }
            }
            251 => {
                let offset_delta = r.read_u2()?;
                StackMapFrame::SameExtended { offset_delta }
            }
            252..=254 => {
                let offset_delta = r.read_u2()?;
                let count = (frame_type - 251) as usize;
                let mut locals = r.vec_with_capacity(count)?;
                for _ in 0..count {
                    locals.push(parse_verification_type_info(r)?);
                }
                StackMapFrame::Append {
                    offset_delta,
                    locals,
                }
            }
            255 => {
                let offset_delta = r.read_u2()?;
                let num_locals = r.read_u2()? as usize;
                let mut locals = r.vec_with_capacity(num_locals)?;
                for _ in 0..num_locals {
                    locals.push(parse_verification_type_info(r)?);
                }
                let num_stack = r.read_u2()? as usize;
                let mut stack = r.vec_with_capacity(num_stack)?;
                for _ in 0..num_stack {
                    stack.push(parse_verification_type_info(r)?);
                }
                StackMapFrame::Full {
                    offset_delta,
                    locals,
                    stack,
                }
            }
            _ => {
                return Err(ClassFileError::InvalidAttribute(
                    "StackMapTable".to_string(),
                ));
            }
        };
        entries.push(frame);
    }
    Ok(StackMapTableAttribute { entries })
}

fn parse_verification_type_info(
    r: &mut Reader<'_, '_>,
) -> Result<VerificationTypeInfo, ClassFileError> {
    let tag = r.read_u1()?;
    let info = match tag {
        0 => VerificationTypeInfo::Top,
        1 => VerificationTypeInfo::Integer,
        2 => VerificationTypeInfo::Float,
        3 => VerificationTypeInfo::Double,
        4 => VerificationTypeInfo::Long,
        5 => VerificationTypeInfo::Null,
        6 => VerificationTypeInfo::UninitializedThis,
        7 => VerificationTypeInfo::Object(r.read_u2()?),
        8 => VerificationTypeInfo::Uninitialized(r.read_u2()?),
        _ => {
            return Err(ClassFileError::InvalidAttribute(
                "StackMapTable".to_string(),
            ));
        }
    };
    Ok(info)
}

fn parse_annotations(r: &mut Reader<'_, '_>) -> Result<Vec<Annotation>, ClassFileError> {
    let num = r.read_u2()? as usize;
    let mut annotations = r.vec_with_capacity(num)?;
    for _ in 0..num {
        annotations.push(parse_annotation(r, 1)?);
    }
    Ok(annotations)
}

fn parse_parameter_annotations(
    r: &mut Reader<'_, '_>,
) -> Result<Vec<Vec<Annotation>>, ClassFileError> {
    let num_parameters = r.read_u1()? as usize;
    let mut out = r.vec_with_capacity(num_parameters)?;
    for _ in 0..num_parameters {
        let num = r.read_u2()? as usize;
        let mut annotations = r.vec_with_capacity(num)?;
        for _ in 0..num {
            annotations.push(parse_annotation(r, 1)?);
        }
        out.push(annotations);
    }
    Ok(out)
}

fn parse_annotation(r: &mut Reader<'_, '_>, depth: usize) -> Result<Annotation, ClassFileError> {
    if depth > r.limits.max_annotation_nesting {
        return Err(ClassFileError::LimitExceeded("annotation nesting"));
    }
    let type_index = r.read_u2()?;
    let num_pairs = r.read_u2()? as usize;
    let mut element_value_pairs = r.vec_with_capacity(num_pairs)?;
    for _ in 0..num_pairs {
        let element_name_index = r.read_u2()?;
        let value = parse_element_value(r, depth)?;
        element_value_pairs.push(ElementValuePair {
            element_name_index,
            value,
        });
    }
    Ok(Annotation {
        type_index,
        element_value_pairs,
    })
}

fn parse_element_value(
    r: &mut Reader<'_, '_>,
    depth: usize,
) -> Result<ElementValue, ClassFileError> {
    if depth > r.limits.max_annotation_nesting {
        return Err(ClassFileError::LimitExceeded("annotation nesting"));
    }
    let tag = r.read_u1()?;
    let value = match tag {
        b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' => {
            let const_value_index = r.read_u2()?;
            ElementValue::Const {
                tag,
                const_value_index,
            }
        }
        b'e' => {
            let type_name_index = r.read_u2()?;
            let const_name_index = r.read_u2()?;
            ElementValue::EnumConst {
                type_name_index,
                const_name_index,
            }
        }
        b'c' => {
            let class_info_index = r.read_u2()?;
            ElementValue::ClassInfo { class_info_index }
        }
        b'@' => {
            let annotation = parse_annotation(r, depth.saturating_add(1))?;
            ElementValue::AnnotationValue(annotation)
        }
        b'[' => {
            let num_values = r.read_u2()? as usize;
            let mut values = r.vec_with_capacity(num_values)?;
            for _ in 0..num_values {
                values.push(parse_element_value(r, depth.saturating_add(1))?);
            }
            ElementValue::ArrayValue(values)
        }
        _ => return Err(ClassFileError::InvalidAttribute("annotation".to_string())),
    };
    Ok(value)
}

fn parse_type_annotations(r: &mut Reader<'_, '_>) -> Result<Vec<TypeAnnotation>, ClassFileError> {
    let num = r.read_u2()? as usize;
    let mut annotations = r.vec_with_capacity(num)?;
    for _ in 0..num {
        let target_type = r.read_u1()?;
        let target_info = parse_target_info(r, target_type)?;
        let target_path = parse_type_path(r)?;
        let type_index = r.read_u2()?;
        let num_pairs = r.read_u2()? as usize;
        let mut element_value_pairs = r.vec_with_capacity(num_pairs)?;
        for _ in 0..num_pairs {
            let element_name_index = r.read_u2()?;
            let value = parse_element_value(r, 1)?;
            element_value_pairs.push(ElementValuePair {
                element_name_index,
                value,
            });
        }
        annotations.push(TypeAnnotation {
            target_type,
            target_info,
            target_path,
            type_index,
            element_value_pairs,
        });
    }
    Ok(annotations)
}

fn parse_target_info(
    r: &mut Reader<'_, '_>,
    target_type: u8,
) -> Result<TargetInfo, ClassFileError> {
    let info = match target_type {
        0x00 | 0x01 => TargetInfo::TypeParameter {
            index: r.read_u1()?,
        },
        0x10 => TargetInfo::Supertype {
            index: r.read_u2()?,
        },
        0x11 | 0x12 => TargetInfo::TypeParameterBound {
            type_parameter_index: r.read_u1()?,
            bound_index: r.read_u1()?,
        },
        0x13..=0x15 => TargetInfo::Empty,
        0x16 => TargetInfo::FormalParameter {
            index: r.read_u1()?,
        },
        0x17 => TargetInfo::Throws {
            index: r.read_u2()?,
        },
        0x40 | 0x41 => {
            let table_length = r.read_u2()? as usize;
            let mut table = r.vec_with_capacity(table_length)?;
            for _ in 0..table_length {
                table.push(LocalVarTarget {
                    start_pc: r.read_u2()?,
                    length: r.read_u2()?,
                    index: r.read_u2()?,
                });
            }
            TargetInfo::Localvar { table }
        }
        0x42 => TargetInfo::Catch {
            exception_table_index: r.read_u2()?,
        },
        0x43..=0x46 => TargetInfo::Offset {
            offset: r.read_u2()?,
        },
        0x47..=0x4B => {
            let offset = r.read_u2()?;
            let type_argument_index = r.read_u1()?;
            TargetInfo::TypeArgument {
                offset,
                type_argument_index,
            }
        }
        _ => {
            return Err(ClassFileError::InvalidAttribute(
                "type_annotation".to_string(),
            ));
        }
    };
    Ok(info)
}

fn parse_type_path(r: &mut Reader<'_, '_>) -> Result<Vec<TypePathEntry>, ClassFileError> {
    let path_length = r.read_u1()? as usize;
    let mut entries = r.vec_with_capacity(path_length)?;
    for _ in 0..path_length {
        entries.push(TypePathEntry {
            type_path_kind: r.read_u1()?,
            type_argument_index: r.read_u1()?,
        });
    }
    Ok(entries)
}

fn parse_module_attribute(r: &mut Reader<'_, '_>) -> Result<ModuleAttribute, ClassFileError> {
    let module_name_index = r.read_u2()?;
    let module_flags = r.read_u2()?;
    let module_version_index = r.read_u2()?;

    let requires_count = r.read_u2()? as usize;
    let mut requires = r.vec_with_capacity(requires_count)?;
    for _ in 0..requires_count {
        requires.push(ModuleRequires {
            requires_index: r.read_u2()?,
            requires_flags: r.read_u2()?,
            requires_version_index: r.read_u2()?,
        });
    }

    let exports_count = r.read_u2()? as usize;
    let mut exports = r.vec_with_capacity(exports_count)?;
    for _ in 0..exports_count {
        let exports_index = r.read_u2()?;
        let exports_flags = r.read_u2()?;
        let exports_to_count = r.read_u2()? as usize;
        let mut exports_to = r.vec_with_capacity(exports_to_count)?;
        for _ in 0..exports_to_count {
            exports_to.push(r.read_u2()?);
        }
        exports.push(ModuleExports {
            exports_index,
            exports_flags,
            exports_to,
        });
    }

    let opens_count = r.read_u2()? as usize;
    let mut opens = r.vec_with_capacity(opens_count)?;
    for _ in 0..opens_count {
        let opens_index = r.read_u2()?;
        let opens_flags = r.read_u2()?;
        let opens_to_count = r.read_u2()? as usize;
        let mut opens_to = r.vec_with_capacity(opens_to_count)?;
        for _ in 0..opens_to_count {
            opens_to.push(r.read_u2()?);
        }
        opens.push(ModuleOpens {
            opens_index,
            opens_flags,
            opens_to,
        });
    }

    let uses_count = r.read_u2()? as usize;
    let mut uses = r.vec_with_capacity(uses_count)?;
    for _ in 0..uses_count {
        uses.push(r.read_u2()?);
    }

    let provides_count = r.read_u2()? as usize;
    let mut provides = r.vec_with_capacity(provides_count)?;
    for _ in 0..provides_count {
        let provides_index = r.read_u2()?;
        let provides_with_count = r.read_u2()? as usize;
        let mut provides_with = r.vec_with_capacity(provides_with_count)?;
        for _ in 0..provides_with_count {
            provides_with.push(r.read_u2()?);
        }
        provides.push(ModuleProvides {
            provides_index,
            provides_with,
        });
    }

    Ok(ModuleAttribute {
        module_name_index,
        module_flags,
        module_version_index,
        requires,
        exports,
        opens,
        uses,
        provides,
    })
}

#[cfg(test)]
mod limit_tests {
    use std::cell::Cell;

    use super::{
        AttributeInfo, ClassFile, ClassFileError, ClassFileParseLimits, ConstantPool, CpInfo,
        JavaString, Reader, parse_attributes, parse_element_value,
    };

    fn encode_attribute(name_index: u16, info: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&1_u16.to_be_bytes());
        encoded.extend_from_slice(&name_index.to_be_bytes());
        encoded.extend_from_slice(&(info.len() as u32).to_be_bytes());
        encoded.extend_from_slice(info);
        encoded
    }

    fn nested_code_attributes(depth: usize) -> Vec<u8> {
        if depth == 0 {
            return 0_u16.to_be_bytes().to_vec();
        }
        let mut code = Vec::new();
        code.extend_from_slice(&0_u16.to_be_bytes());
        code.extend_from_slice(&0_u16.to_be_bytes());
        code.extend_from_slice(&0_u32.to_be_bytes());
        code.extend_from_slice(&0_u16.to_be_bytes());
        code.extend_from_slice(&nested_code_attributes(depth - 1));
        encode_attribute(1, &code)
    }

    fn single_name_pool(name: &str) -> ConstantPool {
        ConstantPool {
            entries: vec![
                None,
                Some(CpInfo::Utf8(JavaString::Unicode(name.to_owned()))),
            ],
        }
    }

    #[test]
    fn nested_annotation_arrays_fail_with_a_typed_limit_error() {
        let limits = ClassFileParseLimits::new().with_max_annotation_nesting(4);
        let mut encoded = Vec::new();
        for _ in 0..5 {
            encoded.extend_from_slice(&[b'[', 0, 1]);
        }
        encoded.extend_from_slice(&[b's', 0, 1]);

        let remaining_allocation_bytes = Cell::new(limits.max_allocation_bytes());
        let mut reader = Reader::new(&encoded, limits, &remaining_allocation_bytes);
        assert!(matches!(
            parse_element_value(&mut reader, 1),
            Err(ClassFileError::LimitExceeded("annotation nesting"))
        ));
    }

    #[test]
    fn nested_collection_capacity_uses_the_shared_allocation_budget() {
        let limits = ClassFileParseLimits::new().with_max_allocation_bytes(1024);
        let remaining_allocation_bytes = Cell::new(limits.max_allocation_bytes());
        let encoded = [b'[', 0xff, 0xff];
        let mut reader = Reader::new(&encoded, limits, &remaining_allocation_bytes);

        assert!(matches!(
            parse_element_value(&mut reader, 1),
            Err(ClassFileError::LimitExceeded("parser allocation budget"))
        ));
    }

    #[test]
    fn oversized_input_fails_before_parsing() {
        let limits = ClassFileParseLimits::new().with_max_input_bytes(4);
        assert!(matches!(
            ClassFile::parse_with_limits(&[0; 5], limits),
            Err(ClassFileError::LimitExceeded("class-file input bytes"))
        ));
    }

    #[test]
    fn recursive_attributes_use_a_separate_depth_budget() {
        let limits = ClassFileParseLimits::new().with_max_attribute_nesting(2);
        let remaining_allocation_bytes = Cell::new(limits.max_allocation_bytes());
        let encoded = nested_code_attributes(3);
        let mut reader = Reader::new(&encoded, limits, &remaining_allocation_bytes);

        assert!(matches!(
            parse_attributes(&mut reader, &single_name_pool("Code"), 1),
            Err(ClassFileError::LimitExceeded("attribute nesting"))
        ));
    }

    #[test]
    fn enclosing_method_consumes_exactly_two_u2_fields() {
        let limits = ClassFileParseLimits::new();
        let remaining_allocation_bytes = Cell::new(limits.max_allocation_bytes());
        let mut info = Vec::new();
        info.extend_from_slice(&2_u16.to_be_bytes());
        info.extend_from_slice(&3_u16.to_be_bytes());
        let encoded = encode_attribute(1, &info);
        let mut reader = Reader::new(&encoded, limits, &remaining_allocation_bytes);

        let attributes = parse_attributes(&mut reader, &single_name_pool("EnclosingMethod"), 1)
            .expect("valid EnclosingMethod attribute");
        assert!(matches!(
            attributes.as_slice(),
            [AttributeInfo::EnclosingMethod {
                class_index: 2,
                method_index: 3
            }]
        ));
    }
}
