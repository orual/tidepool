-- | Vendored aeson — re-exports construction types, lens accessors, and parser.
--
-- Drop-in replacement for Data.Aeson + Data.Aeson.Lens.
-- Uses pure Haskell parser (no attoparsec, no FFI).
module Tidepool.Aeson
  ( -- * Core types (from Tidepool.Aeson.Value)
    Value(..)
  , Key
  , KeyMap
  , Object
  , Array
  , Pair
    -- * Key construction
  , fromText
  , toText
    -- * Value construction
  , object
  , (.=)
  , emptyObject
  , emptyArray
    -- * ToJSON class
  , ToJSON(..)
    -- * Encoding / Decoding
  , encode
  , decode
  , eitherDecode
    -- * Result type
  , Result(..)
    -- * Lens accessors (from Tidepool.Aeson.Lens)
  , key
  , members
  , nth
  , values
  , _String
  , _Number
  , _Bool
  , _Array
  , _Object
  , _Int
  , _Double
  , _Null
  ) where

import Tidepool.Aeson.Value
import Tidepool.Aeson.Lens
