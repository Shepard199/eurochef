import ghidra.app.script.GhidraScript;
public class PhysicsLaunchScalarsEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  long[] a={0x005DD93CL,0x005DD464L,0x005DD72CL,0x00620034L};
  for(long x:a){int v=getInt(toAddr(x));println(String.format("0x%08X bits=0x%08X float=%.12f",x,Integer.toUnsignedLong(v),Float.intBitsToFloat(v)));}
 }
}