module Main where

import Data.Time.Clock.POSIX (getPOSIXTime)
import Data.Word
import Game
import Rng
import Runner

main :: IO ()
main = do
  seriesResult <- runSeries 100

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
  let eloDiff = approxEloDiff (firstWins seriesResult) (secondWins seriesResult)
      eloDiffString = ((if eloDiff > 0 then "+" else "") ++ show eloDiff)

  putStrLn ("Approx. ELO diff of second player vs. first player: " ++ eloDiffString)

  pure ()
