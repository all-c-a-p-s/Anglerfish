module Runner where

import Control.Exception (finally)
import Control.Monad (when)
import Data.Time.Clock.POSIX (getPOSIXTime)
import Data.Word
import Game
import Rng
import System.IO
import System.Process

data Engine = Engine
  { engineIn :: Handle,
    engineOut :: Handle,
    engineProcess :: ProcessHandle
  }

showChipState :: State -> String
showChipState s =
  "sb "
    ++ show (sbStack s)
    ++ " "
    ++ show (sbThisStreet s)
    ++ " bb "
    ++ show (bbStack s)
    ++ " "
    ++ show (bbThisStreet s)

showBetState :: State -> String
showBetState s =
  "pot "
    ++ show (pot s)
    ++ " | SB "
    ++ show (sbStack s)
    ++ " ("
    ++ show (sbThisStreet s)
    ++ ") | BB "
    ++ show (bbStack s)
    ++ " ("
    ++ show (bbThisStreet s)
    ++ ")"

startEngine :: FilePath -> IO Engine
startEngine path = do
  (Just input, Just output, _, process) <-
    createProcess
      (proc path [])
        { std_in = CreatePipe,
          std_out = CreatePipe
        }

  hSetBuffering input LineBuffering
  hSetBuffering output LineBuffering

  pure
    Engine
      { engineIn = input,
        engineOut = output,
        engineProcess = process
      }

stopEngine :: Engine -> IO ()
stopEngine engine = do
  hClose (engineIn engine)
  terminateProcess (engineProcess engine)
  _ <- waitForProcess (engineProcess engine)
  hClose (engineOut engine)

firstPath :: FilePath
firstPath = "./../Anglerfish"

secondPath :: FilePath
secondPath = "./../Anglerfish"

data Action
  = Fold
  | Check
  | Call
  | Quarter
  | Half
  | Pot
  | Raise
  | AllIn
  deriving (Show, Eq, Enum)

send :: Engine -> String -> IO ()
send engine message = do
  hPutStrLn (engineIn engine) message
  hFlush (engineIn engine)

waitForDecision :: Engine -> IO (Action, Maybe String)
waitForDecision engine = do
  line <- hGetLine (engineOut engine)

  case words line of
    "DECISION" : "chose" : "action" : action : "with" : "probability" : p : _ ->
      pure ((parseAction action), Just p)
    "DECISION" : "chose" : "action" : action : _ ->
      pure ((parseAction action), Nothing)
    _ ->
      waitForDecision engine

parseAction :: String -> Action
parseAction s =
  case s of
    "Fold" -> Fold
    "Check" -> Check
    "Call" -> Call
    "Quarter" -> Quarter
    "Half" -> Half
    "Pot" -> Pot
    "2.5x" -> Raise
    "All-in" -> AllIn
    _ -> error ("invalid action: " ++ s)

initHand :: Match -> State -> Engine -> Engine -> IO ()
initHand m s firstEngine secondEngine = do
  send firstEngine (showChipState s)
  send secondEngine (showChipState s)

  send
    firstEngine
    (if firstPos m == SmallBlind then "sb" else "bb")

  send
    secondEngine
    (if firstPos m == SmallBlind then "bb" else "sb")

  send
    firstEngine
    ( if firstPos m == SmallBlind
        then handName (sbHand s)
        else handName (bbHand s)
    )

  send
    secondEngine
    ( if firstPos m == SmallBlind
        then handName (bbHand s)
        else handName (sbHand s)
    )

opponent :: Position -> Position
opponent SmallBlind = BigBlind
opponent BigBlind = SmallBlind

engineFor :: Engine -> Engine -> Position -> Engine
engineFor sbEngine _ SmallBlind = sbEngine
engineFor _ bbEngine BigBlind = bbEngine

stackFor :: Position -> State -> Int
stackFor SmallBlind = sbStack
stackFor BigBlind = bbStack

streetFor :: Position -> State -> Int
streetFor SmallBlind = sbThisStreet
streetFor BigBlind = bbThisStreet

playerName :: Match -> Position -> String
playerName m pos =
  if firstPos m == pos
    then "First"
    else "Second"

check :: State -> State
check s =
  s {turn = opponent (turn s)}

payBet :: State -> Position -> Bool -> Int -> State
payBet s pos enforceMinimum amount =
  case pos of
    SmallBlind ->
      s
        { sbStack = sbStack s - actualAmount,
          sbThisStreet = sbThisStreet s + actualAmount,
          pot = pot s + actualAmount,
          turn = BigBlind,
          maxBet = max (maxBet s) actualAmount
        }
    BigBlind ->
      s
        { bbStack = bbStack s - actualAmount,
          bbThisStreet = bbThisStreet s + actualAmount,
          pot = pot s + actualAmount,
          turn = SmallBlind,
          maxBet = max (maxBet s) actualAmount
        }
  where
    otherPos = opponent pos

    opponentAllIn =
      stackFor otherPos s == 0

    amountToCall =
      max 0 (streetFor otherPos s - streetFor pos s)

    requestedAmount
      | opponentAllIn = amountToCall
      | otherwise = amount

    minimumAdjustedAmount
      | opponentAllIn = requestedAmount
      | enforceMinimum = max (maxBet s) requestedAmount
      | otherwise = requestedAmount

    actualAmount =
      min
        (max 0 minimumAdjustedAmount)
        (stackFor pos s)

refundUncalled :: State -> State
refundUncalled s
  | sbThisStreet s > bbThisStreet s =
      let excess = sbThisStreet s - bbThisStreet s
       in s
            { sbStack = sbStack s + excess,
              sbThisStreet = bbThisStreet s,
              pot = pot s - excess
            }
  | bbThisStreet s > sbThisStreet s =
      let excess = bbThisStreet s - sbThisStreet s
       in s
            { bbStack = bbStack s + excess,
              bbThisStreet = sbThisStreet s,
              pot = pot s - excess
            }
  | otherwise =
      s

behind :: State -> Bool
behind s =
  case turn s of
    SmallBlind ->
      sbThisStreet s < bbThisStreet s
    BigBlind ->
      bbThisStreet s < sbThisStreet s

legal :: State -> Action -> Bool
legal s action =
  if behind s
    then action `elem` [Fold, Call, Raise, AllIn]
    else action `elem` [Check, Quarter, Half, Pot, AllIn]

nextStage :: Stage -> Stage
nextStage Preflop = Flop
nextStage Flop = Turn
nextStage Turn = River
nextStage River = error "tried to advance past the river"

closeStreet :: State -> State
closeStreet s =
  s
    { sbThisStreet = 0,
      bbThisStreet = 0,
      turn = BigBlind,
      maxBet = bigBlind,
      stage = nextStage (stage s)
    }

bettingPossible :: State -> Bool
bettingPossible s =
  sbStack s > 0 && bbStack s > 0

nextDecision ::
  Match ->
  Bool ->
  Engine ->
  Engine ->
  Position ->
  State ->
  IO State
nextDecision m isPreflop sbEngine bbEngine pos t
  | stackFor pos t == 0 =
      pure (refundUncalled t)
  | otherwise =
      do
        let player = engineFor sbEngine bbEngine pos
            otherPos = opponent pos
            otherEngine = engineFor sbEngine bbEngine otherPos

            notifyOther message =
              when (stackFor otherPos t > 0) $
                send otherEngine message

            continue u =
              nextDecision
                m
                isPreflop
                sbEngine
                bbEngine
                otherPos
                u

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

        (dec, p) <- waitForDecision player

        when (not (legal t dec)) $
          error ("illegal action: " ++ show dec)

        let playerMove =
              playerName m pos
                ++ " ("
                ++ show pos
                ++ ") chooses "
                ++ show dec
                ++ case p of
                  Just x -> " with probability " ++ x
                  Nothing -> ""

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

                u = payBet t pos False amount

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

                u = payBet t pos True amount

            putStrLn (showBetState u)
            settleOrContinue u
          AllIn -> do
            notifyOther "all-in"

            let stack = stackFor pos t
                u = payBet t pos False stack

            putStrLn (showBetState u)
            settleOrContinue u

playPreflop :: Match -> State -> Engine -> Engine -> IO State
playPreflop m s firstEngine secondEngine =
  nextDecision m True thisSB thisBB SmallBlind s
  where
    thisSB =
      if firstPos m == SmallBlind
        then firstEngine
        else secondEngine

    thisBB =
      if firstPos m == SmallBlind
        then secondEngine
        else firstEngine

playFlop :: Match -> State -> Engine -> Engine -> IO State
playFlop m s firstEngine secondEngine = do
  let (a, b, c) = flopCards s
      board = unwords (map cardName [a, b, c])

  putStrLn ("Flop: " ++ board)

  if bettingPossible s
    then do
      send firstEngine (cardName a)
      send firstEngine (cardName b)
      send firstEngine (cardName c)

      send secondEngine (cardName a)
      send secondEngine (cardName b)
      send secondEngine (cardName c)

      nextDecision m False thisSB thisBB BigBlind s
    else pure s
  where
    thisSB =
      if firstPos m == SmallBlind then firstEngine else secondEngine

    thisBB =
      if firstPos m == SmallBlind then secondEngine else firstEngine

playTurn :: Match -> State -> Engine -> Engine -> IO State
playTurn m s firstEngine secondEngine = do
  let d = turnCard s

  putStrLn ("Turn: " ++ cardName d)

  if bettingPossible s
    then do
      send firstEngine (cardName d)
      send secondEngine (cardName d)

      nextDecision m False thisSB thisBB BigBlind s
    else pure s
  where
    thisSB =
      if firstPos m == SmallBlind then firstEngine else secondEngine

    thisBB =
      if firstPos m == SmallBlind then secondEngine else firstEngine

playRiver :: Match -> State -> Engine -> Engine -> IO State
playRiver m s firstEngine secondEngine = do
  let e = riverCard s

  putStrLn ("River: " ++ cardName e)

  if bettingPossible s
    then do
      send firstEngine (cardName e)
      send secondEngine (cardName e)

      nextDecision m False thisSB thisBB BigBlind s
    else pure s
  where
    thisSB =
      if firstPos m == SmallBlind then firstEngine else secondEngine

    thisBB =
      if firstPos m == SmallBlind then secondEngine else firstEngine

settleShowdown :: State -> State
settleShowdown s =
  case adjudicateShowdown s of
    SBWins ->
      s
        { sbStack = sbStack s + pot s,
          pot = 0,
          outcome = Just SBWins
        }
    BBWins ->
      s
        { bbStack = bbStack s + pot s,
          pot = 0,
          outcome = Just BBWins
        }
    Draw ->
      let half = pot s `div` 2
          remainder = pot s `mod` 2
       in s
            { sbStack = sbStack s + half + remainder,
              bbStack = bbStack s + half,
              pot = 0,
              outcome = Just Draw
            }

settleFold :: State -> Position -> State
settleFold s folder =
  case folder of
    SmallBlind ->
      s
        { bbStack = bbStack s + pot s,
          pot = 0,
          outcome = Just BBWins
        }
    BigBlind ->
      s
        { sbStack = sbStack s + pot s,
          pot = 0,
          outcome = Just SBWins
        }

firstStack :: Match -> Int
firstStack m =
  case firstPos m of
    SmallBlind -> sbStack (state m)
    BigBlind -> bbStack (state m)

secondStack :: Match -> Int
secondStack m =
  case firstPos m of
    SmallBlind -> bbStack (state m)
    BigBlind -> sbStack (state m)

runMatch :: IO ()
runMatch = do
  t <- getPOSIXTime

  let s =
        State
          { stage = Preflop,
            maxBet = 0,
            bbThisStreet = 0,
            sbThisStreet = 0,
            bbStack = 100,
            sbStack = 100,
            outcome = Nothing,
            turn = SmallBlind,
            deck = cards,
            pot = 0
          }

      seed = floor (t * 1000000) :: Word64

      pos =
        if seed `mod` 2 == 0
          then SmallBlind
          else BigBlind

      m = Match {firstPos = pos, state = s}

  continueMatch m 1
  where
    continueMatch :: Match -> Int -> IO ()
    continueMatch m num = do
      putStrLn (replicate 50 '=')
      putStrLn ("STARTING HAND #" ++ show num)

      putStrLn ""

      putStrLn ("First Player: " ++ firstPath)
      putStrLn ("Second Player: " ++ secondPath)

      putStrLn ""
      putStrLn $
        "Stacks before hand - First "
          ++ show (firstStack m)
          ++ " | Second "
          ++ show (secondStack m)

      putStrLn ""

      endState <- runHand m num

      let finishedMatch = m {state = endState}

      putStrLn $
        "Stacks after hand  - First "
          ++ show (firstStack finishedMatch)
          ++ " | Second "
          ++ show (secondStack finishedMatch)
      putStrLn (replicate 50 '=')
      putStrLn ""

      if firstStack finishedMatch == 0
        then putStrLn "Second player wins"
        else
          if secondStack finishedMatch == 0
            then putStrLn "First player wins"
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

              continueMatch nextMatch (num + 1)

runHand :: Match -> Int -> IO State
runHand m num = do
  firstEngine <- startEngine firstPath
  secondEngine <- startEngine secondPath

  playHand m firstEngine secondEngine
    `finally` do
      stopEngine firstEngine
      stopEngine secondEngine

playHand :: Match -> Engine -> Engine -> IO State
playHand m firstEngine secondEngine = do
  t <- getPOSIXTime

  let s = state m
      seed = floor (t * 1000000) :: Word64
      shuffledDeck = shuffle cards seed

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
        payBet newState SmallBlind False smallBlind

      afterBB =
        payBet afterSB BigBlind False bigBlind

      firstHand =
        if firstPos m == SmallBlind
          then handName (sbHand afterBB)
          else handName (bbHand afterBB)

      secondHand =
        if firstPos m == SmallBlind
          then handName (bbHand afterBB)
          else handName (sbHand afterBB)

  initHand m afterBB firstEngine secondEngine

  putStrLn ("First hand (" ++ show (firstPos m) ++ "): " ++ firstHand)
  putStrLn ("Second hand (" ++ show (opponent (firstPos m)) ++ "): " ++ secondHand)
  putStrLn ("Blinds: " ++ showBetState afterBB)

  endOfPreflop <-
    playPreflop m afterBB firstEngine secondEngine

  case outcome endOfPreflop of
    Just _ ->
      pure endOfPreflop
    Nothing -> do
      let intoFlop = closeStreet endOfPreflop

      endOfFlop <-
        playFlop m intoFlop firstEngine secondEngine

      case outcome endOfFlop of
        Just _ ->
          pure endOfFlop
        Nothing -> do
          let intoTurn = closeStreet endOfFlop

          endOfTurn <-
            playTurn m intoTurn firstEngine secondEngine

          case outcome endOfTurn of
            Just _ ->
              pure endOfTurn
            Nothing -> do
              let intoRiver = closeStreet endOfTurn

              endOfRiver <-
                playRiver m intoRiver firstEngine secondEngine

              case outcome endOfRiver of
                Just _ ->
                  pure endOfRiver
                Nothing -> do
                  let handResult = adjudicateShowdown endOfRiver
                      settled = settleShowdown endOfRiver

                  putStrLn $
                    "Showdown: "
                      ++ case handResult of
                        SBWins -> "small blind wins"
                        BBWins -> "big blind wins"
                        Draw -> "draw"

                  putStrLn (showBetState settled)
                  pure settled
