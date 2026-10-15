import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.address.Address;
public class RobotsInputMember500Evidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 public void run()throws Exception{
  d.openProgram(currentProgram);
  Listing l=currentProgram.getListing();
  Address start=toAddr(0x004d0000L), end=toAddr(0x004d4000L);
  InstructionIterator it=l.getInstructions(start,true);
  java.util.LinkedHashSet<Function> fs=new java.util.LinkedHashSet<>();
  while(it.hasNext()){
   Instruction ins=it.next(); if(ins.getAddress().compareTo(end)>=0)break;
   String s=ins.toString().toLowerCase();
   if(s.contains("+ 0x500]")||s.contains("+0x500]")||s.contains("+ 0x500")||s.contains("+0x500")){
    Function f=getFunctionContaining(ins.getAddress());
    println(ins.getAddress()+" "+ins+" FUNC="+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));
    if(f!=null)fs.add(f);
   }
  }
  for(Function f:fs){
   println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");
   DecompileResults r=d.decompileFunction(f,90,monitor);
   if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  }
  d.dispose();
 }
}