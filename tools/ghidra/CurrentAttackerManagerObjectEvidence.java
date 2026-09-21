import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.Reference;
import java.util.*;

public class CurrentAttackerManagerObjectEvidence extends GhidraScript {
 private void dc(Function f, DecompInterface d)throws Exception{
  if(f==null)return;
  println("\n=== "+f.getName()+"@"+f.getEntryPoint()+" ===");
  DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  Address select=toAddr(0x004563C0L);
  println("=== callers 0x004563C0 ===");
  Set<Function> fs=new LinkedHashSet<>();
  for(Reference ref:getReferencesTo(select)){
    Function f=getFunctionContaining(ref.getFromAddress());
    println(ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
    if(f!=null)fs.add(f);
  }
  for(Function f:fs)dc(f,d);
  d.dispose();
 }
}