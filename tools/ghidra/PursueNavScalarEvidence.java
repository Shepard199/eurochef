import ghidra.app.script.GhidraScript;
public class PursueNavScalarEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  long[] a={0x005E74A4L,0x005E74A8L,0x005E752CL,0x005DD384L,0x005DE538L};
  for(long x:a){int v=getInt(toAddr(x));println(String.format("0x%08X bits=0x%08X float=%.12f",x,Integer.toUnsignedLong(v),Float.intBitsToFloat(v)));}
 }
}