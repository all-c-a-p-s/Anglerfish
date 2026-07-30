module PlayHuman (runHumanMatch) where

import Control.Exception (finally)
import Control.Monad (when)
import Data.Char (toLower)
import Data.Time.Clock.POSIX (getPOSIXTime)
import Data.Word
import Game
import Rng
import Runner
  ( Action (..),
    Engine,
    behind,
    bettingPossible,
    check,
    closeStreet,
    legal,
    opponent,
    payBet,
    refundUncalled,
    send,
    settleFold,
    settleShowdown,
    showBetState,
    showChipState,
    stackFor,
    startEngine,
    stopEngine,
    streetFor,
    waitForDecision,
  )
import System.IO (hFlush, stdout)

enginePath :: FilePath
enginePath = "./Anglerfish"

parseHumanAction :: String -> Maybe Action
parseHumanAction input =
  case map toLower input of
    "fold" -> Just Fold
    "check" -> Just Check
    "call" -> Just Call
    "quarter" -> Just Quarter
    "half" -> Just Half
    "pot" -> Just Pot
    "2.5x" -> Just Raise
    "all-in" -> Just AllIn
    _ -> Nothing

humanDecision :: State -> IO (Action, Maybe String)
humanDecision s = do
  putStrLn $
    if behind s
      then "Choose: fold, call, 2.5x, all-in"
      else "Choose: check, quarter, half, pot, all-in"

  putStr "> "
  hFlush stdout
  input <- getLine

  case parseHumanAction input of
    Just action
      | legal s action ->
          pure (action, Nothing)
      | otherwise -> do
          putStrLn "That action is not legal here."
          humanDecision s
    Nothing -> do
      putStrLn "Unrecognised action."
      humanDecision s

humanPosition :: Match -> Position
humanPosition = firstPos

enginePosition :: Match -> Position
enginePosition m =
  opponent (firstPos m)

humanStack :: Match -> Int
humanStack m =
  stackFor (humanPosition m) (state m)

engineStack :: Match -> Int
engineStack m =
  stackFor (enginePosition m) (state m)

playerName :: Match -> Position -> String
playerName m pos =
  if pos == humanPosition m
    then "Human"
    else "Engine"

initHumanHand :: Match -> State -> Engine -> IO ()
initHumanHand m s engine = do
  send engine (showChipState s)

  send
    engine
    (if enginePosition m == SmallBlind then "sb" else "bb")

  send
    engine
    ( if enginePosition m == SmallBlind
        then handName (sbHand s)
        else handName (bbHand s)
    )

nextHumanDecision ::
  Match ->
  Bool ->
  Engine ->
  Position ->
  State ->
  IO State
nextHumanDecision m isPreflop engine pos t
  | stackFor pos t == 0 =
      pure (refundUncalled t)
  | otherwise = do
      let otherPos =
            opponent pos

          playerIsHuman =
            pos == humanPosition m

          otherIsEngine =
            otherPos == enginePosition m

          notifyOther message =
            when (otherIsEngine && stackFor otherPos t > 0) $
              send engine message

          continue =
            nextHumanDecision
              m
              isPreflop
              engine
              otherPos

          bettingClosedByAllIn u =
            stackFor otherPos t == 0
              || ( stackFor pos u == 0
                     && streetFor pos u
                       <= streetFor otherPos u
                 )

          settleOrContinue u =
            if bettingClosedByAllIn u
              then pure (refundUncalled u)
              else continue u

      (dec, _) <-
        if playerIsHuman
          then humanDecision t
          else waitForDecision engine

      if legal t dec
        then pure ()
        else error ("illegal action: " ++ show dec)

      let playerMove =
            playerName m pos
              ++ " ("
              ++ show pos
              ++ ") chooses "
              ++ show dec

      putStrLn playerMove

      case dec of
        Fold -> do
          notifyOther "fold"

          let u = settleFold t pos

          putStrLn (showBetState u)
          pure u
        Check -> do
          notifyOther "check"

          let u = check t

          putStrLn (showBetState u)

          if stackFor otherPos t == 0
            || isPreflop
            || pos == SmallBlind
            then pure (refundUncalled u)
            else continue u
        Call -> do
          notifyOther "call"

          let amount =
                streetFor otherPos t
                  - streetFor pos t

              wasRaised =
                max
                  (sbThisStreet t)
                  (bbThisStreet t)
                  > bigBlind

              u =
                payBet t pos False amount

              someoneAllIn =
                sbStack u == 0 || bbStack u == 0

          putStrLn (showBetState u)

          if someoneAllIn
            || not isPreflop
            || wasRaised
            then pure (refundUncalled u)
            else continue u
        Quarter -> do
          notifyOther "quarter"

          let amount = pot t `div` 4
              u = payBet t pos True amount

          putStrLn (showBetState u)
          settleOrContinue u
        Half -> do
          notifyOther "half"

          let amount = pot t `div` 2
              u = payBet t pos True amount

          putStrLn (showBetState u)
          settleOrContinue u
        Pot -> do
          notifyOther "pot"

          let amount = pot t
              u = payBet t pos True amount

          putStrLn (showBetState u)
          settleOrContinue u
        Raise -> do
          notifyOther "2.5x"

          let opponentPaid =
                streetFor otherPos t

              alreadyPaid =
                streetFor pos t

              target =
                opponentPaid * 5 `div` 2

              amount =
                target - alreadyPaid

              u =
                payBet t pos True amount

          putStrLn (showBetState u)
          settleOrContinue u
        AllIn -> do
          notifyOther "all-in"

          let stack = stackFor pos t
              u = payBet t pos False stack

          putStrLn (showBetState u)
          settleOrContinue u

playHumanPreflop ::
  Match ->
  State ->
  Engine ->
  IO State
playHumanPreflop m s engine =
  nextHumanDecision
    m
    True
    engine
    SmallBlind
    s

playHumanFlop ::
  Match ->
  State ->
  Engine ->
  IO State
playHumanFlop m s engine = do
  let (a, b, c) = flopCards s
      board = unwords (map cardName [a, b, c])

  putStrLn ("Board: " ++ board)

  if bettingPossible s
    then do
      send engine (cardName a)
      send engine (cardName b)
      send engine (cardName c)

      nextHumanDecision
        m
        False
        engine
        BigBlind
        s
    else pure s

playHumanTurn ::
  Match ->
  State ->
  Engine ->
  IO State
playHumanTurn m s engine = do
  let (a, b, c) = flopCards s
      d = turnCard s
      board = unwords (map cardName [a, b, c, d])

  putStrLn ("Board: " ++ board)

  if bettingPossible s
    then do
      send engine (cardName d)

      nextHumanDecision
        m
        False
        engine
        BigBlind
        s
    else pure s

playHumanRiver ::
  Match ->
  State ->
  Engine ->
  IO State
playHumanRiver m s engine = do
  let (a, b, c) = flopCards s
      d = turnCard s
      e = riverCard s
      board = unwords (map cardName [a, b, c, d, e])

  putStrLn ("Board: " ++ board)

  if bettingPossible s
    then do
      send engine (cardName e)

      nextHumanDecision
        m
        False
        engine
        BigBlind
        s
    else pure s

runHumanMatch :: IO ()
runHumanMatch = do
  t <- getPOSIXTime

  let s =
        State
          { stage = Preflop,
            maxBet = 0,
            bbThisStreet = 0,
            sbThisStreet = 0,
            bbStack = startingStack,
            sbStack = startingStack,
            outcome = Nothing,
            turn = SmallBlind,
            deck = cards,
            pot = 0
          }

      seed =
        floor (t * 1000000) :: Word64

      pos =
        if even seed
          then SmallBlind
          else BigBlind

      m =
        Match
          { firstPos = pos,
            state = s
          }

  continueHumanMatch m 1
  where
    continueHumanMatch :: Match -> Int -> IO ()
    continueHumanMatch m handNumber = do
      putStrLn (replicate 50 '=')
      putStrLn ("STARTING HAND #" ++ show handNumber)
      putStrLn (replicate 50 '=')
      putStrLn ""

      putStrLn "Human: you"
      putStrLn ("Engine: " ++ enginePath)

      putStrLn ""

      putStrLn $
        "Stacks before hand - Human "
          ++ show (humanStack m)
          ++ " | Engine "
          ++ show (engineStack m)

      putStrLn ""

      endState <-
        runHumanHand m

      let finishedMatch =
            m {state = endState}

      putStrLn $
        "Stacks after hand  - Human "
          ++ show (humanStack finishedMatch)
          ++ " | Engine "
          ++ show (engineStack finishedMatch)

      putStrLn (replicate 50 '=')
      putStrLn ""

      if humanStack finishedMatch == 0
        then
          putStrLn "Engine wins the match."
        else
          if engineStack finishedMatch == 0
            then
              putStrLn "Human wins the match!"
            else do
              let nextState =
                    endState
                      { sbStack = bbStack endState,
                        bbStack = sbStack endState
                      }

                  nextMatch =
                    Match
                      { firstPos = opponent (firstPos m),
                        state = nextState
                      }

              continueHumanMatch
                nextMatch
                (handNumber + 1)

runHumanHand :: Match -> IO State
runHumanHand m = do
  engine <- startEngine enginePath

  playHumanHand m engine
    `finally` stopEngine engine

playHumanHand ::
  Match ->
  Engine ->
  IO State
playHumanHand m engine = do
  t <- getPOSIXTime

  let s = state m

      seed =
        floor (t * 1000000) :: Word64

      shuffledDeck =
        shuffle cards seed

      newState =
        State
          { sbStack = sbStack s,
            sbThisStreet = 0,
            bbStack = bbStack s,
            bbThisStreet = 0,
            pot = pot s,
            deck = shuffledDeck,
            turn = SmallBlind,
            outcome = Nothing,
            maxBet = bigBlind,
            stage = Preflop
          }

      afterSB =
        payBet
          newState
          SmallBlind
          False
          smallBlind

      afterBB =
        payBet
          afterSB
          BigBlind
          False
          bigBlind

      yourHand =
        if humanPosition m == SmallBlind
          then handName (sbHand afterBB)
          else handName (bbHand afterBB)

  initHumanHand m afterBB engine

  putStrLn $
    "You are the "
      ++ show (humanPosition m)
      ++ "."

  putStrLn ("Your hand: " ++ yourHand)
  putStrLn ("Blinds: " ++ showBetState afterBB)
  putStrLn ""

  endOfPreflop <-
    playHumanPreflop
      m
      afterBB
      engine

  case outcome endOfPreflop of
    Just _ ->
      pure endOfPreflop
    Nothing -> do
      let intoFlop =
            closeStreet endOfPreflop

      endOfFlop <-
        playHumanFlop
          m
          intoFlop
          engine

      case outcome endOfFlop of
        Just _ ->
          pure endOfFlop
        Nothing -> do
          let intoTurn =
                closeStreet endOfFlop

          endOfTurn <-
            playHumanTurn
              m
              intoTurn
              engine

          case outcome endOfTurn of
            Just _ ->
              pure endOfTurn
            Nothing -> do
              let intoRiver =
                    closeStreet endOfTurn

              endOfRiver <-
                playHumanRiver
                  m
                  intoRiver
                  engine

              case outcome endOfRiver of
                Just _ ->
                  pure endOfRiver
                Nothing -> do
                  let handResult =
                        adjudicateShowdown endOfRiver

                      settled =
                        settleShowdown endOfRiver

                      humanWon =
                        handResult
                          == if humanPosition m == SmallBlind
                            then SBWins
                            else BBWins

                      engineHand =
                        if enginePosition m == SmallBlind
                          then handName (sbHand endOfRiver)
                          else handName (bbHand endOfRiver)

                  putStrLn ("Engine hand: " ++ engineHand)

                  putStrLn $
                    case handResult of
                      Draw ->
                        "Showdown: draw"
                      _
                        | humanWon ->
                            "Showdown: human wins"
                        | otherwise ->
                            "Showdown: engine wins"

                  putStrLn (showBetState settled)
                  pure settled
