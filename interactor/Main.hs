module Main where

import Data.Time.Clock.POSIX (getPOSIXTime)
import Data.Word
import Game
import Rng
import Runner

main :: IO ()
main = do
  runMatch
