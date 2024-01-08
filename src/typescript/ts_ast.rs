use std::fmt::{self, Write};

trait FmtTS {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result;
}

/// A small wrapper macro to implement Display using a type's FmtLua
/// implementation. We can apply this to values that we want to stringify
/// directly.
macro_rules! proxy_display {
    ( $target: ty ) => {
        impl fmt::Display for $target {
            fn fmt(&self, output: &mut fmt::Formatter) -> fmt::Result {
                let mut stream = TSStream::new(output);
                FmtTS::fmt_ts(self, &mut stream)
            }
        }
    };
}

pub(crate) enum ModifierToken {
    Export,
    Readonly,
    Declare,
}
impl ModifierToken {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Export => "export",
            Self::Declare => "declare",
            Self::Readonly => "readonly",
        }
    }
}

pub(crate) struct FunctionType {
    parameters: Vec<Parameter>,
    return_type: Box<Expression>,
}
impl FunctionType {
    pub fn new(parameters: Vec<Parameter>, return_type: Expression) -> FunctionType {
        FunctionType {
            parameters,
            return_type: Box::new(return_type),
        }
    }
}
impl FmtTS for FunctionType {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result {
        write!(output, "(")?;

        // for parameter in &self.parameters {
        //     parameter.fmt_ts(output)?;
        // }
        self.parameters.fmt_ts(output)?;

        write!(output, ") => ")?;
        self.return_type.fmt_ts(output)
    }
}

pub(crate) struct PropertySignature {
    name: String,
    modifiers: Option<Vec<ModifierToken>>,
    expression: Expression,
}
impl PropertySignature {
    pub fn new(name: String, modifiers: Option<Vec<ModifierToken>>, expression: Expression) -> PropertySignature {
        PropertySignature {
            name,
            modifiers,
            expression,
        }
    }
}
impl FmtTS for PropertySignature {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result {
        if let Some(modifiers) = &self.modifiers {
            for modifier in modifiers {
                write!(output, "{} ", modifier.as_str())?;
            }
        }

        if self.name.chars().all(char::is_alphanumeric) && self.name.chars().nth(0).unwrap().is_alphabetic() {
            writeln!(output, "{}: {};", self.name, self.expression)
        } else {
            writeln!(output, "[\"{}\"]: {};", self.name, self.expression)
        }
    }
}

pub(crate) enum TypeReference {
    Expression(Expression),
    Union(Vec<TypeReference>),
}
impl FmtTS for TypeReference {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result {
        match self {
            TypeReference::Expression(inner) => {
                write!(output, "{}", inner)?;
            },
            TypeReference::Union(types) => {
                let count = types.len();
                let mut iter = 0;

                for parameter in types {
                    iter += 1;
                    
                    parameter.fmt_ts(output)?;
        
                    if iter < count {
                        write!(output, " | ")?;
                    }
                }
            },
        }

        Ok(())
    }
}
impl TypeReference {
    pub fn id(id: String) -> TypeReference {
        TypeReference::Expression(Expression::Identifier(id))
    }

    pub fn num(value: i32) -> TypeReference {
        TypeReference::Expression(Expression::NumericLiteral(value))
    }

    pub fn union(inner: Vec<TypeReference>) -> TypeReference {
        TypeReference::Union(inner)
    }
}

pub(crate) struct Parameter {
    name: String,
    param_type: TypeReference,
}

impl Parameter {
    pub fn new(name: String, param_type: TypeReference) -> Parameter {
        Parameter {
            name,
            param_type,
        }
    }
}

impl FmtTS for Vec<Parameter> {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result {
        let count = self.len();
        let mut iter = 0;

        for parameter in self {
            iter += 1;

            write!(output, "{}: ", parameter.name)?;
            parameter.param_type.fmt_ts(output)?;

            if iter < count {
                write!(output, ", ")?;
            }
        }

        Ok(())
    }
}

pub enum VariableKind {
    Const,
}

pub(crate) struct ExportAssignment {
    expression: Expression,
}

pub(crate) struct VariableDeclaration {
    name: String,
    kind: VariableKind,
    type_expression: Option<Expression>,
    modifiers: Option<Vec<ModifierToken>>,
    expression: Option<Expression>,
}
impl FmtTS for VariableDeclaration {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result {
        if let Some(mod_tokens) = &self.modifiers {
            for mod_token in mod_tokens {
                write!(output, "{} ", mod_token.as_str())?;
            }
        }

        write!(
            output,
            "{} {}",
            match self.kind {
                VariableKind::Const => "const",
            },
            self.name
        )?;

        if let Some(type_expression) = &self.type_expression {
            write!(output, ": ")?;
            type_expression.fmt_ts(output)?;
        }

        if let Some(expression) = &self.expression {
            write!(output, " = ")?;
            expression.fmt_ts(output)?;
        }

        writeln!(output, ";")
    }
}

pub(crate) struct InterfaceDeclaration {
    name: String,
    modifiers: Option<Vec<ModifierToken>>,
    members: Vec<PropertySignature>,
}
impl InterfaceDeclaration {
    pub fn new(
        name: String,
        modifiers: Option<Vec<ModifierToken>>,
        members: Vec<PropertySignature>,
    ) -> InterfaceDeclaration {
        InterfaceDeclaration {
            name,
            modifiers,
            members,
        }
    }
}
impl FmtTS for InterfaceDeclaration {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result {
        if let Some(mod_tokens) = &self.modifiers {
            for mod_token in mod_tokens {
                write!(output, "{} ", mod_token.as_str())?;
            }
        }

        writeln!(output, "interface {} {{", self.name)?;

        output.indent();
        for signature in &self.members {
            signature.fmt_ts(output)?;
        }
        output.unindent();

        writeln!(output, "}}")
    }
}

pub(crate) enum Expression {
    Identifier(String),
    StringLiteral(String),
    NumericLiteral(i32),
    TypeLiteral(Vec<PropertySignature>),
    FunctionType(FunctionType),
}
impl FmtTS for Expression {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result {
        match self {
            Self::Identifier(ident) => {
                write!(output, "{}", ident)
            }
            Self::StringLiteral(literal) => {
                write!(output, "\"{}\"", literal)
            }
            Self::NumericLiteral(literal) => {
                write!(output, "{}", literal)
            }
            Self::TypeLiteral(literal) => {
                writeln!(output, "{{")?;

                output.indent();
                for signature in literal {
                    signature.fmt_ts(output)?;
                }
                output.unindent();
                write!(output, "}}")?;

                Ok(())
            }
            Self::FunctionType(func) => func.fmt_ts(output),
        }
    }
}
proxy_display!(Expression);

pub(crate) enum Statement {
    InterfaceDeclaration(InterfaceDeclaration),
    VariableDeclaration(VariableDeclaration),
    ExportAssignment(ExportAssignment),
}

impl Statement {
    pub fn export_assignment(expression: Expression) -> Statement {
        Statement::ExportAssignment(ExportAssignment { expression })
    }
}

impl FmtTS for Statement {
    fn fmt_ts(&self, output: &mut TSStream) -> fmt::Result {
        match self {
            Self::InterfaceDeclaration(declaration) => declaration.fmt_ts(output),
            Self::VariableDeclaration(declaration) => declaration.fmt_ts(output),
            Self::ExportAssignment(export) => {
                writeln!(output, "export = {};", export.expression)
            }
        }
    }
}
proxy_display!(Statement);

pub(crate) struct TSStream<'a> {
    indent_level: usize,
    is_start_of_line: bool,
    inner: &'a mut (dyn fmt::Write + 'a),
}

impl<'a> TSStream<'a> {
    pub fn new(inner: &'a mut (dyn fmt::Write + 'a)) -> Self {
        Self {
            indent_level: 0,
            is_start_of_line: true,
            inner,
        }
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn unindent(&mut self) {
        assert!(self.indent_level > 0);
        self.indent_level -= 1;
    }

    fn line(&mut self) -> fmt::Result {
        self.is_start_of_line = true;
        self.inner.write_str("\n")
    }
}

impl fmt::Write for TSStream<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let mut is_first_line = true;

        for line in value.split('\n') {
            if is_first_line {
                is_first_line = false;
            } else {
                self.line()?;
            }

            if !line.is_empty() {
                if self.is_start_of_line {
                    self.is_start_of_line = false;
                    let indentation = "\t".repeat(self.indent_level);
                    self.inner.write_str(&indentation)?;
                }

                self.inner.write_str(line)?;
            }
        }

        Ok(())
    }
}
