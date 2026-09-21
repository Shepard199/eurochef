import ghidra.app.script.GhidraScript;
public class PiranhaScalarEvidence extends GhidraScript {
  @Override public void run() throws Exception {
    long[] a={0x005DD3C8L,0x005DD388L,0x005DD380L,0x005DD384L,0x005DD3BCL,0x005DD3C4L,0x005DE534L,0x005DE538L};
    for(long x:a) println(String.format("0x%08X bits=0x%08X float=%s",x,Integer.toUnsignedLong(getInt(toAddr(x))),Float.intBitsToFloat(getInt(toAddr(x)))));
  }
}