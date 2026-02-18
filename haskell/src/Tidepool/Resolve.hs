module Tidepool.Resolve (resolveExternals) where

import GHC.Core (CoreBind, CoreExpr, Bind(..), maybeUnfoldingTemplate)
import GHC.Core.FVs (exprFreeVars)
import GHC.Types.Id (Id, idUnfolding, isGlobalId, isPrimOpId_maybe, isDataConWorkId_maybe)
import GHC.Types.Var (Var)
import GHC.Types.Var.Set (VarSet, emptyVarSet, unitVarSet, unionVarSet, elemVarSet, extendVarSet)
import GHC.Types.Unique.Set (nonDetEltsUniqSet)

-- | Resolve cross-module references by inlining their unfoldings.
--
-- Starting from the free variables in the given CoreBinds, look up
-- unfoldings for any global (non-local) Ids. If an unfolding is
-- available, add it as a NonRec binding and recurse on its free
-- variables. Primops, data constructors, and Ids without unfoldings
-- are left as-is (the evaluator handles them via env_from_datacon_table
-- and dispatch_primop).
--
-- Returns the original bindings with resolved externals prepended.
resolveExternals :: [CoreBind] -> [CoreBind]
resolveExternals binds =
  let localBinders = foldMap bindersOfSet binds
      allFreeVars  = foldMap freeVarsOfBind binds
      -- External free vars: global, not local, not primops, not data constructors
      externals    = filter (isResolvable localBinders) (nonDetEltsUniqSet allFreeVars)
      (resolved, _) = go externals emptyVarSet localBinders []
  in resolved ++ binds
  where
    go :: [Var] -> VarSet -> VarSet -> [CoreBind] -> ([CoreBind], VarSet)
    go [] visited _ acc = (reverse acc, visited)
    go (v:rest) visited localSet acc
      | elemVarSet v visited = go rest visited localSet acc
      | otherwise =
          let visited' = extendVarSet visited v
          in case maybeUnfoldingTemplate (idUnfolding v) of
               Nothing ->
                 -- No unfolding available — leave as external
                 go rest visited' localSet acc
               Just unfoldingExpr ->
                 let newBind = NonRec v unfoldingExpr
                     newFVs = exprFreeVars unfoldingExpr
                     localSet' = extendVarSet localSet v
                     newExternals = filter (isResolvable localSet')
                                          (nonDetEltsUniqSet newFVs)
                 in go (newExternals ++ rest) visited' localSet' (newBind : acc)

    isResolvable :: VarSet -> Var -> Bool
    isResolvable localSet v =
      isGlobalId v
      && not (elemVarSet v localSet)
      && not (isPrimOp v)
      && not (isDataCon v)

    bindersOfSet :: CoreBind -> VarSet
    bindersOfSet (NonRec b _) = unitVarSet b
    bindersOfSet (Rec pairs) = foldl (\s (b, _) -> extendVarSet s b) emptyVarSet pairs

    freeVarsOfBind :: CoreBind -> VarSet
    freeVarsOfBind (NonRec _ rhs) = exprFreeVars rhs
    freeVarsOfBind (Rec pairs) = foldMap (exprFreeVars . snd) pairs

    isPrimOp :: Id -> Bool
    isPrimOp v = case isPrimOpId_maybe v of
      Just _  -> True
      Nothing -> False

    isDataCon :: Id -> Bool
    isDataCon v = case isDataConWorkId_maybe v of
      Just _  -> True
      Nothing -> False
