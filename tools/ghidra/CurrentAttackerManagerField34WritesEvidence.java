import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import java.util.*;

public class CurrentAttackerManagerField34WritesEvidence extends GhidraScript {
  @Override public void run() throws Exception {
    Address start=toAddr(0x00456000L), end=toAddr(0x00456700L);
    InstructionIterator it=currentProgram.getListing().getInstructions(start,true);
    Set<Function> funcs=new LinkedHashSet<>();
    while(it.hasNext()){
      Instruction ins=it.next();
      if(ins.getAddress().compareTo(end)>=0) break;
      String s=ins.toString().toLowerCase();
      int comma=s.indexOf(',');
      if(comma<0) continue;
      String dst=s.substring(0,comma);
      if(!(dst.contains("+ 0x34]")||dst.contains("+0x34]"))) continue;
      if(!(s.startsWith("mov ")||s.startsWith("lea ")||s.startsWith("xchg ")||s.startsWith("and ")||s.startsWith("or "))) continue;
      Function f=getFunctionContaining(ins.getAddress());
      println(ins.getAddress()+"  "+ins+"  FUNC="+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
      if(f!=null) funcs.add(f);
    }
    DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
    for(Function f:funcs){
      println("\n=== "+f.getName()+"@"+f.getEntryPoint()+" ===");
      DecompileResults r=d.decompileFunction(f,90,monitor);
      if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
    }
    d.dispose();
  }
}
