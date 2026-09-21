import ghidra.app.script.GhidraScript;
public class PiranhaMoreScalarsEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  long[] a={0x005DD940L,0x005DD72CL,0x00620034L,0x005DFF08L,0x005DDEA4L};
  for(long x:a) println(String.format("0x%08X bits=0x%08X float=%s",x,Integer.toUnsignedLong(getInt(toAddr(x))),Float.intBitsToFloat(getInt(toAddr(x)))));
 }
}