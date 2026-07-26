module Rng where

import Data.Bits
import Data.Word

next :: Word64 -> Word64
next x =
  let x1 = xor (shiftL x 13) x
      x2 = xor (shiftR x1 7) x1
      x3 = xor (shiftL x2 17) x2
   in x3

replace :: Int -> a -> [a] -> [a]
replace 0 new (_ : xs) = new : xs
replace n new (x : xs) = x : replace (n - 1) new xs
replace _ _ [] = []

swap :: Int -> Int -> [a] -> [a]
swap i j l =
  let u = l !! i
      v = l !! j
      l1 = replace i v l
      l2 = replace j u l1
   in l2

shuffle :: [a] -> Word64 -> [a]
shuffle [] _ = []
shuffle (x : []) _ = [x]
shuffle list state =
  let r = next state
      i = (fromIntegral r) `mod` (length list)
      l1 = swap 0 i list
   in case l1 of
        y : ys -> y : shuffle ys r
        [] -> []
