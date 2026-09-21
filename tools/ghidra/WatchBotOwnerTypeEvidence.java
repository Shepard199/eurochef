import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class WatchBotOwnerTypeEvidence extends GhidraScript {
 @Override public void run()throws Exception{
  Address a=toAddr(0x004AFF80L);disassemble(a);Function f=getFunctionContaining(a);
  println("FUNCTION="+(f==null?"<missing>":f.getName()+"@"+f.getEntryPoint()));
  if(f==null)return;DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  DecompileResults r=d.decompileFunction(f,150,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  d.dispose();
 }
}