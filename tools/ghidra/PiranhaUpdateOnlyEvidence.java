import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class PiranhaUpdateOnlyEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address a=toAddr(0x004676F0L); disassemble(a);
  Function f=getFunctionAt(a); if(f==null) f=createFunction(a,"tmp_Piranha_Update");
  println("FUNCTION="+(f==null?"<missing>":f.getName()+"@"+f.getEntryPoint()));
  if(f==null)return;
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
  d.dispose();
 }
}