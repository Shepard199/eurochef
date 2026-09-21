import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class PiranhaHeightTransitionDeepEvidence extends GhidraScript {
 private void recover(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw); disassemble(a); Function f=getFunctionAt(a);
  if(f==null) f=createFunction(a,name);
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run() throws Exception {
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  recover(0x00467B50L,"tmp_Piranha_HeightTransition",d);
  recover(0x00448950L,"tmp_SoundDispatch_A",d);
  recover(0x00448AF0L,"tmp_SoundDispatch_B",d);
  d.dispose();
 }
}