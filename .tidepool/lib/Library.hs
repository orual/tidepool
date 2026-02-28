module Library where

-- | Hylomorphism: unfold then fold in one pass
hylo :: (b -> c -> c) -> c -> (a -> Maybe (b, a)) -> a -> c
hylo f z g seed = case g seed of
  Nothing      -> z
  Just (b, a') -> f b (hylo f z g a')

-- | Paramorphism: fold with access to the remaining tail
para :: (a -> [a] -> b -> b) -> b -> [a] -> b
para f z []     = z
para f z (x:xs) = f x xs (para f z xs)

-- | Anamorphism: unfold from a seed
ana :: (a -> Maybe (b, a)) -> a -> [b]
ana f seed = case f seed of
  Nothing      -> []
  Just (b, a') -> b : ana f a'

-- | Catamorphism: standard fold (just foldr, but named for symmetry)
cata :: (a -> b -> b) -> b -> [a] -> b
cata = foldr

-- | Monadic hylomorphism: unfold then fold with effects
hyloM :: Monad m => (b -> c -> m c) -> c -> (a -> m (Maybe (b, a))) -> a -> m c
hyloM f z g seed = do
  r <- g seed
  case r of
    Nothing      -> pure z
    Just (b, a') -> do
      c <- hyloM f z g a'
      f b c

-- | Monadic anamorphism: effectful unfold
anaM :: Monad m => (a -> m (Maybe (b, a))) -> a -> m [b]
anaM f seed = do
  r <- f seed
  case r of
    Nothing      -> pure []
    Just (b, a') -> (b :) <$> anaM f a'

-- | Monadic paramorphism: effectful fold with tail access
paraM :: Monad m => (a -> [a] -> b -> m b) -> b -> [a] -> m b
paraM f z []     = pure z
paraM f z (x:xs) = do
  r <- paraM f z xs
  f x xs r

-- | Apomorphism: unfold that can short-circuit with a complete tail
apo :: (a -> Either [b] (b, a)) -> a -> [b]
apo f seed = case f seed of
  Left bs      -> bs
  Right (b, a') -> b : apo f a'

-- | Monadic apomorphism: effectful unfold with early bail-out
apoM :: Monad m => (a -> m (Either [b] (b, a))) -> a -> m [b]
apoM f seed = do
  r <- f seed
  case r of
    Left bs       -> pure bs
    Right (b, a') -> (b :) <$> apoM f a'

-- | Zygomorphism: two folds in one pass (auxiliary + main)
zygo :: (a -> b -> b) -> (a -> (b, c) -> c) -> b -> c -> [a] -> c
zygo f g bz cz []     = cz
zygo f g bz cz (x:xs) =
  let b = f x (cata f bz xs)
      c = zygo f g bz cz xs
  in  g x (b, c)

-- | Tree hylomorphism: unfold into a binary tree, then fold it
--   alg combines (left_result, value, right_result) -> result
--   coalg splits seed into Left leaf_value or Right (left_seed, value, right_seed)
treeHylo :: (c -> b -> c -> c) -> (a -> c) -> (a -> Either a (a, b, a)) -> a -> c
treeHylo alg leaf coalg seed = case coalg seed of
  Left a           -> leaf a
  Right (l, b, r)  -> alg (treeHylo alg leaf coalg l) b (treeHylo alg leaf coalg r)

-- | Iterate n times
iterateN :: Int -> (a -> a) -> a -> [a]
iterateN 0 f x = []
iterateN n f x = x : iterateN (n - 1) f (f x)

-- | Fixed-point iteration: apply until stable
converge :: Eq a => (a -> a) -> a -> a
converge f x = let x' = f x in if x == x' then x else converge f x'

-- ---------------------------------------------------------------------------
-- Van Laarhoven lenses (work with existing ^?, .~, %~, & from Prelude)
-- ---------------------------------------------------------------------------

-- | Build a lens from getter and setter
lens :: (s -> a) -> (s -> a -> s) -> Functor f => (a -> f a) -> s -> f s
lens get set f s = (\a -> set s a) <$> f (get s)

-- | Lens into first element of a pair
_1 :: Functor f => (a -> f a) -> (a, b) -> f (a, b)
_1 f (a, b) = (\a' -> (a', b)) <$> f a

-- | Lens into second element of a pair
_2 :: Functor f => (b -> f b) -> (a, b) -> f (a, b)
_2 f (a, b) = (\b' -> (a, b')) <$> f b

-- | Lens into list element by index
ix :: Int -> Functor f => (a -> f a) -> [a] -> f [a]
ix i f xs = case splitAt i xs of
  (before, x:after) -> (\x' -> before ++ [x'] ++ after) <$> f x
  _                 -> (\_ -> xs) <$> f (head xs)  -- noop on out of bounds

-- ---------------------------------------------------------------------------
-- Scan: like fold but collects all intermediate accumulator values
-- ---------------------------------------------------------------------------

-- | Left scan collecting intermediate states
scanl' :: (b -> a -> b) -> b -> [a] -> [b]
scanl' f z []     = [z]
scanl' f z (x:xs) = z : scanl' f (f z x) xs

-- | Monadic left scan
scanlM :: Monad m => (b -> a -> m b) -> b -> [a] -> m [b]
scanlM f z []     = pure [z]
scanlM f z (x:xs) = do
  z' <- f z x
  rest <- scanlM f z' xs
  pure (z : rest)

-- ---------------------------------------------------------------------------
-- Bounded iteration (safe for JIT's ~20 depth limit)
-- ---------------------------------------------------------------------------

-- | Iterate with fuel: stops after n steps or when predicate says stop
iterateWhile :: (a -> Bool) -> (a -> a) -> a -> [a]
iterateWhile p f x = if p x then x : iterateWhile p f (f x) else []

-- | Monadic iterate with fuel
iterateWhileM :: Monad m => (a -> Bool) -> (a -> m a) -> a -> m [a]
iterateWhileM p f x = if p x
  then do { x' <- f x; rest <- iterateWhileM p f x'; pure (x : rest) }
  else pure []

-- | Apply f until predicate holds, return final value
until' :: (a -> Bool) -> (a -> a) -> a -> a
until' p f x = if p x then x else until' p f (f x)

-- | Monadic until
untilM :: Monad m => (a -> Bool) -> (a -> m a) -> a -> m a
untilM p f x = if p x then pure x else f x >>= untilM p f
