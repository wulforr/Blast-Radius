use oxc_allocator::Allocator;
use oxc_ast::ast::{
    CallExpression, ExportAllDeclaration, ExportFromDeclaration, Expression, ImportDeclaration,
    ImportExpression,
};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    Static,
    ExportFrom,
    Require,
    DynamicLiteral,
    /// `import(someVariable)` — the specifier is not knowable statically.
    /// Recorded as a gap rather than dropped.
    DynamicExpression,
}

#[derive(Debug, Clone)]
pub struct ImportRef {
    pub specifier: Option<String>,
    pub kind: ImportKind,
    pub line: u32,
}

#[derive(Debug, Default)]
pub struct ParsedFile {
    pub imports: Vec<ImportRef>,
    pub parse_failed: bool,
}

pub fn parse_imports(source: &str, path: &str) -> ParsedFile {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_default();
    let result = Parser::new(&allocator, source, source_type).parse();

    let mut collector = Collector {
        source,
        imports: Vec::new(),
    };
    collector.visit_program(&result.program);

    ParsedFile {
        imports: collector.imports,
        parse_failed: !result.diagnostics.is_empty(),
    }
}

struct Collector<'s> {
    source: &'s str,
    imports: Vec<ImportRef>,
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        self.imports.push(ImportRef {
            specifier: Some(it.source.value.as_str().to_string()),
            kind: ImportKind::Static,
            line: line_of(self.source, it.span.start),
        });
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
        self.imports.push(ImportRef {
            specifier: Some(it.source.value.as_str().to_string()),
            kind: ImportKind::ExportFrom,
            line: line_of(self.source, it.span.start),
        });
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        self.imports.push(ImportRef {
            specifier: Some(it.source.value.as_str().to_string()),
            kind: ImportKind::ExportFrom,
            line: line_of(self.source, it.span.start),
        });
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        match &it.source {
            Expression::StringLiteral(lit) => self.imports.push(ImportRef {
                specifier: Some(lit.value.as_str().to_string()),
                kind: ImportKind::DynamicLiteral,
                line: line_of(self.source, lit.span.start),
            }),
            _ => self.imports.push(ImportRef {
                specifier: None,
                kind: ImportKind::DynamicExpression,
                line: line_of(self.source, it.span.start),
            }),
        }
        walk::walk_import_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Expression::Identifier(callee) = &it.callee {
            if callee.name.as_str() == "require" {
                let literal = it
                    .arguments
                    .first()
                    .and_then(|arg| arg.as_expression())
                    .and_then(|expr| match expr {
                        Expression::StringLiteral(lit) => Some(lit),
                        _ => None,
                    });
                if let Some(lit) = literal {
                    self.imports.push(ImportRef {
                        specifier: Some(lit.value.as_str().to_string()),
                        kind: ImportKind::Require,
                        line: line_of(self.source, lit.span.start),
                    });
                }
            }
        }
        walk::walk_call_expression(self, it);
    }
}

fn line_of(source: &str, offset: u32) -> u32 {
    let end = (offset as usize).min(source.len());
    // 1-based, matching every editor and every CI annotation format.
    source
        .get(..end)
        .map_or(1, |s| s.matches('\n').count() as u32 + 1)
}
