
func Test() {
  if true
    && false
    && true
    && false
    && true
    && false {
  } else if false {
    FTLog("a");
  } else {
    FTLog("b");
  }

  if true {
    FTLog("c");
  }

  while true
    && false
    && true
    && false
    && true
    && false {
    FTLog("d");
  }

  while false {
    FTLog("e");
  }

  for x in [
    "lorem",
    "ipsum",
    "dolor",
    "sit",
    "amet",
    "consectetur",
    "adipiscing",
    "elit",
    "sed",
    "do",
    "eiusmod",
    "tempor",
    "incididunt",
    "ut",
    "labore",
    "et",
    "dolore",
    "magna",
    "aliqua"
  ] {
    FTLog(x);
  }

  switch 1 {
    case 0:
      FTLog("f");
    case 1:
      FTLog("g");
    default:
      FTLog("h");
  }

  let result;
  switch [1, 2, 3] {
    case let [.., a, b, c]:
      result = a;
      break;
    case let [a, b, ..]:
      result =  b;
      break;
    case let [a]:
      result = a;
      break;
    default:
      result = 0;
  }

  if let [.., a, b, c] = [1, 2, 3] {
    result = a;
  }

  if let [one, two, three, four, five, six, seven, eight, nine, ten, eleven, twelve, thirteen, fourteen, fifteen, sixteen] =
    [ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16 ] {
  }

  if let Class { fieldA: fieldA } = new Class() {
  }

  if let Class { fieldA, fieldB, fieldC, fieldD, fieldE, fieldF, fieldG, fieldH, fieldI, fieldJ, fieldK } = new Class() {
  }

  for y in [1, 2, 3] {
    FTLog(s"f: \(y)");
  }

  let f1 = (a) -> a;
  let f2 = (a) -> {
    return a;
  };
  let f3 = (a: Int32) -> a;
}

class Class {
  let fieldA: Int8;
  let fieldB: Int16;
  let fieldC: Int32;
  let fieldD: Int64;
  let fieldE: Uint8;
  let fieldF: Uint16;
  let fieldG: Uint32;
  let fieldH: Uint64;
  let fieldI: Float;
  let fieldJ: Double;
  let fieldK: String;
}

// trailing comment
