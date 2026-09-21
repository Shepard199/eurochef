import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class PiranhaUpdateCallsiteDecompile extends GhidraScript {
 @Override public void run() throws Exception {
  Address a=toAddr(0x004676F0L); disassemble(a); Function f=getFunctionAt(a);
  if(f==null) f=createFunction(a,"tmp_Piranha_Update");
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null){
   String c=r.getDecompiledFunction().getC();
   int i=c.indexOf("FUN_00467b50");
   println(i<0?c:c.substring(Math.max(0,i-2200),Math.min(c.length(),i+1200)));
  }
  d.dispose();
 }
}