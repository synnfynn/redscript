use std::cell::Cell;
use std::rc::Rc;

use redscript_ast as ast;
use redscript_ast::Span;

use crate::lower::error::{Error, LowerResult};
use crate::lower::types::PolyType;
use crate::symbols::{FreeFunctionIndex, FreeFunctionIndexes, FunctionEntry, Symbols};
use crate::types::{CtxVar, Type, TypeId, predef};
use crate::utils::{Lazy, ScopedMap};
use crate::{FreeFunction, IndexMap, ir};

#[derive(Debug)]
pub struct Env<'scope, 'ctx> {
    types: &'scope TypeEnv<'scope, 'ctx>,
    funcs: &'scope ScopedMap<'scope, &'ctx str, FreeFunctionIndexes>,
    locals: ScopedMap<'scope, &'ctx str, ir::LocalInfo<'ctx>>,
}

impl<'scope, 'ctx> Env<'scope, 'ctx> {
    pub fn new(
        types: &'scope TypeEnv<'scope, 'ctx>,
        globals: &'scope ScopedMap<'scope, &'ctx str, FreeFunctionIndexes>,
    ) -> Self {
        Self {
            types,
            funcs: globals,
            locals: ScopedMap::default(),
        }
    }

    #[inline]
    pub fn define_local(&mut self, name: &'ctx str, info: ir::LocalInfo<'ctx>) -> ir::Local {
        let id = info.id;
        self.locals.insert(name, info);
        id
    }

    #[inline]
    pub fn introduce_scope(&'scope self) -> Self {
        Self {
            types: self.types,
            funcs: self.funcs,
            locals: self.locals.introduce_scope(),
        }
    }

    pub fn query_free_functions<'a>(
        &self,
        name: &'a str,
        symbols: &'scope Symbols<'ctx>,
    ) -> impl Iterator<Item = FunctionEntry<FreeFunctionIndex, &'a str, &'scope FreeFunction<'ctx>>>
    {
        self.funcs
            .scope_iter()
            .filter_map(|map| map.get(name))
            .flatten()
            .map(move |&idx| FunctionEntry::new(idx, name, &symbols[idx]))
    }

    pub fn locals(&self) -> &ScopedMap<'scope, &'ctx str, ir::LocalInfo<'ctx>> {
        &self.locals
    }

    pub fn types(&self) -> &TypeEnv<'scope, 'ctx> {
        self.types
    }
}

#[derive(Debug, Clone)]
pub struct TypeEnv<'scope, 'ctx>(ScopedMap<'scope, &'ctx str, TypeRef<'scope, 'ctx>>);

impl<'scope, 'ctx> TypeEnv<'scope, 'ctx> {
    pub fn with_default_types() -> Self {
        let mut map = IndexMap::default();
        map.insert("array", TypeRef::Name(predef::ARRAY));
        map.insert("ref", TypeRef::Name(predef::REF));
        map.insert("wref", TypeRef::Name(predef::WREF));
        map.insert("script_ref", TypeRef::Name(predef::SCRIPT_REF));

        Self(ScopedMap::from(map))
    }

    #[inline]
    pub fn introduce_scope(&'scope self) -> Self {
        Self(self.0.introduce_scope())
    }

    #[inline]
    pub fn pop_scope(self) -> IndexMap<&'ctx str, TypeRef<'scope, 'ctx>> {
        self.0.pop_scope()
    }

    #[inline]
    pub fn push_scope(&'scope self, scope: IndexMap<&'ctx str, TypeRef<'scope, 'ctx>>) -> Self {
        Self(self.0.push_scope(scope))
    }

    #[inline]
    pub fn top(&self) -> &IndexMap<&'ctx str, TypeRef<'scope, 'ctx>> {
        self.0.top()
    }

    #[inline]
    pub fn get(&self, name: &'ctx str) -> Option<&TypeRef<'scope, 'ctx>> {
        self.0.get(name)
    }

    #[inline]
    pub fn add(&mut self, name: &'ctx str, typ: TypeRef<'scope, 'ctx>) {
        self.0.insert(name, typ);
    }

    pub fn resolve(
        &self,
        typ: &ast::SourceType<'ctx>,
        span: Span,
    ) -> LowerResult<'ctx, Type<'ctx>> {
        match typ {
            ast::Type::Named { name, args } => match (self.0.get(name), &args[..]) {
                (Some(&TypeRef::Name(id)), [(arg, span)]) if id == predef::REF => {
                    Ok(self.resolve(arg, *span)?)
                }
                (Some(&TypeRef::Name(id)), _) => {
                    let args = args
                        .iter()
                        .map(|(typ, span)| self.resolve(typ, *span))
                        .collect::<Result<Rc<_>, _>>()?;
                    Ok(Type::app(id, args))
                }
                (Some(TypeRef::Var(var)), _) => Ok(Type::Ctx(var.clone())),
                (Some(TypeRef::LazyVar(stub)), _) => Ok(Type::Ctx(
                    stub.get(self).map_err(|_| Error::CyclicType(span))??,
                )),
                (None, _) => Err(Error::UnresolvedType(name, span)),
            },
            ast::Type::Array(elem) => {
                let (elem, span) = &**elem;
                let elem = self.resolve(elem, *span)?;
                Ok(Type::app(predef::ARRAY, [elem]))
            }
            ast::Type::StaticArray(elem, size) => {
                let (elem, elem_span) = &**elem;
                let elem = self.resolve(elem, *elem_span)?;
                let id = TypeId::array_with_size(*size)
                    .ok_or(Error::UnsupportedStaticArraySize(span))?;
                Ok(Type::app(id, [elem]))
            }
            ast::Type::Fn {
                params,
                return_type: return_typ,
            } => {
                let args = params
                    .iter()
                    .chain([&**return_typ])
                    .map(|(typ, span)| self.resolve(typ, *span))
                    .collect::<Result<Rc<_>, _>>()?;

                let id =
                    TypeId::fn_with_arity(params.len()).ok_or(Error::UnsupportedArity(span))?;
                Ok(Type::app(id, args))
            }
        }
    }

    pub fn resolve_param(
        &self,
        param: &ast::SourceTypeParam<'ctx>,
    ) -> LowerResult<'ctx, CtxVar<'ctx>> {
        let (name, _) = param.name;
        let variance = param.variance.into();
        let upper = param
            .upper_bound
            .as_deref()
            .map(|(typ, span)| self.resolve(typ, *span))
            .transpose()?;
        Ok(CtxVar::new(name, variance, None, upper))
    }
}

impl Default for TypeEnv<'_, '_> {
    fn default() -> Self {
        Self::with_default_types()
    }
}

#[derive(Debug)]
pub struct Locals<'scope, 'ctx> {
    locals: Vec<ir::LocalInfo<'ctx>>,
    counter: &'scope Cell<u16>,
    depth: usize,
}

impl<'scope, 'ctx> Locals<'scope, 'ctx> {
    #[inline]
    pub fn new(counter: &'scope Cell<u16>, depth: usize) -> Self {
        Self {
            locals: vec![],
            counter,
            depth,
        }
    }

    #[inline]
    pub fn add_var(&mut self, typ: PolyType<'ctx>, span: Span) -> &ir::LocalInfo<'ctx> {
        self.add(ir::Local::Var(self.counter.get()), typ, Some(span))
    }

    #[inline]
    pub fn add_param(&mut self, typ: PolyType<'ctx>, span: Option<Span>) -> &ir::LocalInfo<'ctx> {
        self.add(ir::Local::Param(self.counter.get()), typ, span)
    }

    pub fn into_vec(self) -> Vec<ir::LocalInfo<'ctx>> {
        self.locals
    }

    pub fn counter(&self) -> &'scope Cell<u16> {
        self.counter
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    #[inline]
    fn add(
        &mut self,
        local: ir::Local,
        typ: PolyType<'ctx>,
        span: Option<Span>,
    ) -> &ir::LocalInfo<'ctx> {
        self.counter.set(self.counter.get() + 1);
        let len = self.locals.len();
        self.locals.push(ir::LocalInfo::new(local, typ, span));
        &self.locals[len]
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Capture {
    pub captured: ir::Local,
    pub depth: u16,
}

impl Capture {
    #[inline]
    pub fn new(captured: ir::Local, depth: u16) -> Self {
        Self { captured, depth }
    }

    #[inline]
    pub fn pop_scope(&self) -> Option<Self> {
        Some(Self::new(self.captured, self.depth.checked_sub(1)?))
    }
}

#[derive(Debug, Clone)]
pub enum TypeRef<'scope, 'ctx> {
    Name(TypeId<'ctx>),
    Var(Rc<CtxVar<'ctx>>),
    #[allow(clippy::type_complexity)]
    LazyVar(
        Rc<
            Lazy<
                LowerResult<'ctx, Rc<CtxVar<'ctx>>>,
                Box<dyn Fn(&TypeEnv<'_, 'ctx>) -> LowerResult<'ctx, Rc<CtxVar<'ctx>>> + 'scope>,
            >,
        >,
    ),
}

impl<'ctx> TypeRef<'_, 'ctx> {
    pub fn force(self) -> Option<TypeRef<'static, 'ctx>> {
        match self {
            Self::Name(id) => Some(TypeRef::Name(id)),
            Self::Var(typ) => Some(TypeRef::Var(typ)),
            Self::LazyVar(lazy) => Some(TypeRef::Var(lazy.try_get()?.ok()?)),
        }
    }

    pub fn id(&self) -> Option<TypeId<'ctx>> {
        match self {
            &Self::Name(id) => Some(id),
            _ => None,
        }
    }
}
