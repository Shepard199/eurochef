import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class CharacterPhysics4193A0Evidence extends GhidraScript {
 @Override public void run() throws Exception {
  Function f=getFunctionContaining(toAddr(0x004193A0L));
  println(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint());
  if(f==null)return;
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  DecompileResults r=d.decompileFunction(f,60,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
  d.dispose();
 }
}