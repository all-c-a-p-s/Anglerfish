module Main where

import Data.Time.Clock.POSIX (getPOSIXTime)
import Data.Word
import Game
import Rng
import Runner

main :: IO ()
main = do
  seriesResult <- runSeries 10

  putStrLn ""
  putStrLn ""

  putStrLn ("FINAL RESULT:")
  putStrLn
    ( "("
        ++ firstPath
        ++ " "
        ++ show (firstWins seriesResult)
        ++ ") - ("
        ++ secondPath
        ++ " "
        ++ show (secondWins seriesResult)
        ++ ")"
    )

  pure ()
