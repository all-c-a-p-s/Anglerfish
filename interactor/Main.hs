module Main where

import Data.Time.Clock.POSIX (getPOSIXTime)
import Data.Word
import Game
import PlayHuman
import Rng
import Runner

playHuman :: Bool
playHuman = True

main :: IO ()
main = do
  if playHuman
    then do
      runHumanMatch
    else do
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
