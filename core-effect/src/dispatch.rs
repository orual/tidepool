use crate::error::EffectError;
use core_bridge::{FromCore, ToCore};
use core_eval::value::Value;
use core_repr::DataConTable;
use frunk::{HCons, HNil};

pub struct EffectContext<'a> {
    table: &'a DataConTable,
}

impl<'a> EffectContext<'a> {
    pub(crate) fn new(table: &'a DataConTable) -> Self {
        Self { table }
    }

    pub fn respond<T: ToCore>(&self, val: T) -> Result<Value, EffectError> {
        val.to_value(self.table).map_err(EffectError::Bridge)
    }

    pub fn table(&self) -> &DataConTable {
        self.table
    }
}

pub trait EffectHandler {
    type Request: FromCore;
    fn handle(
        &mut self,
        req: Self::Request,
        cx: &EffectContext,
    ) -> Result<Value, EffectError>;
}

pub trait DispatchEffect {
    fn dispatch(
        &mut self,
        tag: u64,
        request: &Value,
        table: &DataConTable,
    ) -> Result<Value, EffectError>;
}

impl DispatchEffect for HNil {
    fn dispatch(
        &mut self,
        tag: u64,
        _request: &Value,
        _table: &DataConTable,
    ) -> Result<Value, EffectError> {
        Err(EffectError::UnhandledEffect { tag })
    }
}

impl<H: EffectHandler, T: DispatchEffect> DispatchEffect for HCons<H, T> {
    fn dispatch(
        &mut self,
        tag: u64,
        request: &Value,
        table: &DataConTable,
    ) -> Result<Value, EffectError> {
        if tag == 0 {
            let req = H::Request::from_value(request, table)?;
            let cx = EffectContext::new(table);
            self.head.handle(req, &cx)
        } else {
            self.tail.dispatch(tag - 1, request, table)
        }
    }
}
