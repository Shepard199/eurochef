import ghidra.app.script.GhidraScript;
public class PiranhaPitchScalarEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  long a=0x005DD734L; int v=getInt(toAddr(a));
  println(String.format("0x%08X bits=0x%08X float=%.12f",a,Integer.toUnsignedLong(v),Float.intBitsToFloat(v)));
 }
}