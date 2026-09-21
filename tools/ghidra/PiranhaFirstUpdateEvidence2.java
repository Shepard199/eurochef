import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class PiranhaFirstUpdateEvidence2 extends GhidraScript {
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  Function f=getFunctionAt(toAddr(0x00467C60L));if(f==null)f=getFunctionContaining(toAddr(0x00467C60L));
  DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  int bits=getInt(toAddr(0x005DD940L));println(String.format("DAT_005DD940 bits=%08X value=%s",bits,Float.toString(Float.intBitsToFloat(bits))));
  d.dispose();
 }
}